use std::{
    convert::TryInto,
    ffi::CString,
    io,
    mem::{transmute, zeroed, MaybeUninit},
    ptr::{self, copy_nonoverlapping},
};

use bytes::{BufMut, BytesMut};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

#[derive(Debug)]
pub struct Request {
    typ: u8,
    idt: u16,
    seq: u16,
    dat: Vec<u8>,
}

impl Request {
    pub fn new(typ: u8, idt: u16, seq: u16, len: usize) -> Self {
        Self {
            typ,
            idt,
            seq,
            dat: Vec::with_capacity(len),
        }
    }

    pub fn set_ident(&mut self, idt: u16) -> &mut Self {
        self.idt = idt;
        self
    }

    pub fn set_sequence(&mut self, seq: u16) -> &mut Self {
        self.seq = seq;
        self
    }

    pub fn put_data(&mut self, data: &[u8]) -> &mut Self {
        assert!(self.dat.capacity() - self.len() >= data.len());
        self.dat.extend_from_slice(&data);
        self
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buffer = BytesMut::with_capacity(self.len());
        buffer.put_u8(self.typ);
        buffer.put_u8(0);
        buffer.put_u16(0);
        buffer.put_u16(self.idt);
        buffer.put_u16(self.seq);
        buffer.put_slice(&self.dat);
        if self.dat.len() < self.dat.capacity() {
            buffer.put_slice(&vec![0; self.dat.capacity() - self.dat.len()]);
        }

        let sum = checksum(&buffer).to_be_bytes();
        let mut res = buffer.to_vec();
        res[2] = sum[0];
        res[3] = sum[1];

        res
    }

    #[inline]
    pub fn len(&self) -> usize {
        8 + self.dat.len()
    }
}

#[derive(Debug)]
pub struct Response {
    typ: u8,
    cod: u8,
    sum: u16,
    idt: u16,
    seq: u16,
    pub dat: Box<[u8]>,
}

impl Response {
    pub fn decode(bytes: &[u8]) -> Self {
        let typ = bytes[0];
        let cod = bytes[1];
        let sum = u16::from_be_bytes(bytes[2..2 + 2].try_into().unwrap());
        let idt = u16::from_be_bytes(bytes[4..4 + 2].try_into().unwrap());
        let seq = u16::from_be_bytes(bytes[6..6 + 2].try_into().unwrap());

        let dat_len = bytes.len() - 8;
        let mut dat = Vec::with_capacity(dat_len);
        dat.extend_from_slice(&bytes[8..]);

        Self {
            typ,
            cod,
            sum,
            idt,
            seq,
            dat: dat.into_boxed_slice(),
        }
    }
}

pub fn checksum(bytes: &[u8]) -> u16 {
    let mut sum = 0u32;
    let skip = 1;
    bytes.chunks_exact(2).enumerate().for_each(|(i, buf)| {
        if i != skip {
            sum += u16::from_be_bytes(buf.try_into().unwrap()) as u32;
        }
    });

    while sum >> 16 != 0 {
        sum = (sum >> 16) + (sum & 0xffff);
    }

    !sum as u16
}

#[inline]
pub fn decode(bytes: &[u8]) -> Response {
    let header_len = 4 * (bytes[0] & 0xf) as usize;
    Response::decode(&bytes[header_len..])
}

#[derive(Debug)]
pub enum Version {
    V4,
    V6,
}

#[derive(Debug)]
pub struct Icmp {
    pub socket: socket2::Socket,
    dst: socket2::SockAddr,
    ver: Version,
}

impl Icmp {
    pub fn new(ver: Version, dst: &str) -> Self {
        let socket = match ver {
            Version::V4 => Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4)).unwrap(),
            Version::V6 => unimplemented!(),
        };

        let (_, dst) = unsafe {
            SockAddr::init(|addr, len| {
                let mut res = ptr::null_mut();
                let mut hints: libc::addrinfo = zeroed();
                match ver {
                    Version::V4 => hints.ai_family = libc::AF_INET,
                    Version::V6 => hints.ai_family = libc::AF_INET6,
                }

                let host = CString::new(dst).unwrap();
                libc::getaddrinfo(host.as_ptr(), ptr::null(), &hints, &mut res);
                len.write((*res).ai_addrlen);
                copy_nonoverlapping((*res).ai_addr, addr.cast(), 1);
                libc::freeaddrinfo(res);
                Ok(())
            })
        }
        .unwrap();

        Self { socket, dst, ver }
    }

    #[inline]
    pub fn send(&self, req: &Request) -> io::Result<usize> {
        self.socket.send_to(&req.encode(), &self.dst)
    }

    pub fn recv(&self, packet_len: usize) -> io::Result<(usize, SockAddr, Response)> {
        let len = packet_len + 60;
        let mut dat = vec![MaybeUninit::uninit(); len];
        let (len, addr) = self.socket.recv_from(&mut dat)?;
        let recv_dat: &[u8] = unsafe { transmute(&dat[..len]) };

        let resp = decode(recv_dat);
        Ok((len, addr, resp))
    }
}

#[cfg(test)]
mod tests {
    // TODO: Implement test code
}
