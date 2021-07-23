use std::{convert::TryInto, mem::zeroed, ptr, thread::sleep, time::Duration};

use icmp::{Icmp, Request};

#[macro_export]
macro_rules! syscall {
    ($fun:ident ( $($arg:expr),* $(,)* )) => {
        {
            let res = unsafe { libc::$fun($($arg),*) };
            if res == -1 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(res)
            }
        }
    };
}

fn main() {
    let icmp = Icmp::new(icmp::Version::V4, "www.baidu.com");

    let mut time = unsafe { zeroed() };
    for i in 0..8 {
        let mut req = Request::new(8, 99, i, 64);
        syscall!(gettimeofday(&mut time, ptr::null_mut())).unwrap();
        let now = time.tv_sec as u64 * 1000 + time.tv_usec as u64 / 1000;
        req.put_data(&now.to_be_bytes());
        icmp.send(&req).unwrap();
        let (len, addr, resp) = icmp.recv(64).unwrap();
        let pass = u64::from_be_bytes(resp.dat[..8].try_into().unwrap());
        syscall!(gettimeofday(&mut time, ptr::null_mut())).unwrap();
        let now = time.tv_sec as u64 * 1000 + time.tv_usec as u64 / 1000;
        println!(
            "len: {}, recv from: {:?}, resp: {:x?}, time: {}ms",
            len,
            addr.as_socket_ipv4(),
            resp,
            now - pass
        );
        sleep(Duration::from_secs(1));
    }
}
