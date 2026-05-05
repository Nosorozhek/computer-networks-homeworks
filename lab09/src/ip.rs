use pnet::datalink;

fn main() {
    for interface in datalink::interfaces() {
        if !interface.ips.is_empty() && !interface.is_loopback() {
            println!("Interface: {}", interface.name);
            for ip_network in &interface.ips {
                println!("  IP Address: {}", ip_network.ip());
                println!("  Netmask:    {}", ip_network.mask());
            }
        }
    }
}
