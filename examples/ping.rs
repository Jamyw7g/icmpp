use std::{thread::sleep, time::Duration};

use icmpp::{Icmp, Version};

fn main() {
    let mut icmp = Icmp::new(Version::V4, "www.baidu.com", 99, None).unwrap();
    for _ in 0..8 {
        icmp.send().unwrap();
        let (len, addr, resp) = icmp.recv().unwrap();
        println!(
            "total len: {}, packet len: {}, recv from: {:?}, resp: {:02x?}",
            len,
            resp.len(),
            addr.as_socket_ipv4(),
            resp
        );

        sleep(Duration::from_secs(1));
    }
}
