use std::{
    convert::TryInto,
    ffi::CString,
    io,
    mem::{transmute, zeroed, MaybeUninit},
    ptr::{self, copy_nonoverlapping},
};

use bytes::{BufMut, BytesMut};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

pub const DEFDATALEN: usize = 56;
pub const MAXIPLEN: usize = 60;
pub const MAXSEQ: u16 = u16::MAX;

#[derive(Debug)]
pub struct Response {
    typ: u8,
    cod: u8,
    sum: u16,
    idt: u16,
    seq: u16,
    dat: Box<[u8]>,
}

impl Response {
    pub fn decode(bytes: &[u8]) -> Self {
        let typ = bytes[0];
        let cod = bytes[1];
        let sum = u16::from_be_bytes(bytes[2..4].try_into().unwrap());
        let idt = u16::from_be_bytes(bytes[4..6].try_into().unwrap());
        let seq = u16::from_be_bytes(bytes[6..8].try_into().unwrap());
        let dat = Vec::from(&bytes[8..]);

        Self {
            typ,
            cod,
            sum,
            idt,
            seq,
            dat: dat.into_boxed_slice(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        8 + self.dat.len()
    }

    #[inline]
    pub fn kind(&self) -> u8 {
        self.typ
    }

    #[inline]
    pub fn code(&self) -> u8 {
        self.cod
    }

    #[inline]
    pub fn checksum(&self) -> u16 {
        self.sum
    }

    #[inline]
    pub fn ident(&self) -> u16 {
        self.idt
    }

    #[inline]
    pub fn sequence(&self) -> u16 {
        self.seq
    }

    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.dat
    }
}

#[derive(Debug)]
pub enum Version {
    V4,
    V6,
}

#[derive(Debug)]
pub struct Icmp {
    pub sock: Socket,
    dst: SockAddr,
    ver: Version,
    typ: u8,
    idt: u16,
    seq: u16,
    dat: Box<[u8]>,
}

impl Icmp {
    pub fn new(ver: Version, dst: &str, idt: u16, len: Option<usize>) -> io::Result<Self> {
        let sock = match ver {
            Version::V4 => Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4))?,
            Version::V6 => Socket::new(Domain::IPV6, Type::RAW, Some(Protocol::ICMPV6))?,
        };
        let len = len.unwrap_or(DEFDATALEN);
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
                if res.is_null() {
                    return Err(std::io::Error::last_os_error());
                }
                len.write((*res).ai_addrlen);
                copy_nonoverlapping((*res).ai_addr, addr.cast(), 1);
                libc::freeaddrinfo(res);
                Ok(())
            })
        }
        .unwrap();

        Ok(Self {
            sock,
            dst,
            ver,
            typ: 8,
            idt,
            seq: 0,
            dat: vec![0; len].into_boxed_slice(),
        })
    }

    pub fn send(&mut self) -> io::Result<usize> {
        let mut buf = BytesMut::with_capacity(self.serialize_len());
        buf.put_u8(self.typ);
        buf.put_u8(0);
        buf.put_u16(0);
        buf.put_u16(self.idt);
        buf.put_u16(self.seq);
        buf.put_slice(&self.dat);

        let sum = checksum(&buf).to_be_bytes();
        let mut buf = buf.to_vec();
        buf[2] = sum[0];
        buf[3] = sum[1];

        let len = self.sock.send_to(&buf, &self.dst)?;
        self.seq = (self.seq + 1) % MAXSEQ;
        Ok(len)
    }

    pub fn recv(&self) -> io::Result<(usize, SockAddr, Response)> {
        let mut buf = vec![MaybeUninit::uninit(); MAXIPLEN + self.serialize_len()];

        loop {
            let (len, addr) = self.sock.recv_from(&mut buf)?;
            let dat: &[u8] = unsafe { transmute(&buf[..len]) };
            let ip_hdr_len = 4 * (dat[0] & 0xf) as usize;
            let idt = u16::from_be_bytes(dat[ip_hdr_len + 4..ip_hdr_len + 6].try_into().unwrap());
            if idt != self.idt {
                continue;
            }
            let resp = Response::decode(&dat[ip_hdr_len..]);

            return Ok((len, addr, resp));
        }
    }

    #[inline]
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.dat
    }

    #[inline]
    pub fn serialize_len(&self) -> usize {
        8 + self.dat.len()
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

#[cfg(test)]
mod tests {
    // TODO: Implement test code
}
