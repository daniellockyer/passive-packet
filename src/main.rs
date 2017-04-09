extern crate pnet;
extern crate iron;
extern crate staticfile;
extern crate mount;

use std::path::Path;
use std::thread;

use iron::Iron;
use staticfile::Static;
use mount::Mount;

use pnet::datalink::{self, NetworkInterface};
use pnet::packet::Packet;
use pnet::packet::arp::ArpPacket;
use pnet::datalink::Channel::Ethernet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use std::net::IpAddr;

fn handle_packet(interface_name: &str, ethernet: &EthernetPacket) {
    match ethernet.get_ethertype() {
        EtherTypes::Ipv4 => handle_ipv4_packet(interface_name, ethernet),
        EtherTypes::Ipv6 => handle_ipv6_packet(interface_name, ethernet),
        EtherTypes::Arp => handle_arp_packet(interface_name, ethernet),
        _ => println!("[{}]: Unknown packet: {} > {}; ethertype: {:?} length: {}",
				interface_name, ethernet.get_source(), ethernet.get_destination(), 
				ethernet.get_ethertype(), ethernet.packet().len())
    }
}

fn handle_arp_packet(interface_name: &str, ethernet: &EthernetPacket) {
    let header = ArpPacket::new(ethernet.payload());
    if let Some(header) = header {
        println!("[{}]: ARP packet: {}({}) > {}({}); operation: {:?}",
                 interface_name,
                 ethernet.get_source(),
                 header.get_sender_proto_addr(),
                 ethernet.get_destination(),
                 header.get_target_proto_addr(),
                 header.get_operation());
    } else {
        println!("[{}]: Malformed ARP Packet", interface_name);
    }
}

fn handle_ipv4_packet(interface_name: &str, ethernet: &EthernetPacket) {
    let header = Ipv4Packet::new(ethernet.payload());
    if let Some(header) = header {
        println!("{:?} {:?} {:?} {:?}", interface_name, IpAddr::V4(header.get_source()),
        	IpAddr::V4(header.get_destination()), header.get_next_level_protocol());
    } else {
        println!("[{}]: Malformed IPv4 Packet", interface_name);
    }
}

fn handle_ipv6_packet(interface_name: &str, ethernet: &EthernetPacket) {
    let header = Ipv6Packet::new(ethernet.payload());
    if let Some(header) = header {
        println!("{:?} {:?} {:?} {:?}", interface_name, IpAddr::V6(header.get_source()),
        	IpAddr::V6(header.get_destination()), header.get_next_header());
    } else {
        println!("[{}]: Malformed IPv6 Packet", interface_name);
    }
}

fn main() {
    for interface in datalink::interfaces() {
        let mac = interface.mac.map(|mac| mac.to_string()).unwrap_or_else(|| "N/A".to_owned());
        println!("{}:", interface.name);
        println!("\tindex: {}", interface.index);
        println!("\tflags: {}", interface.flags);
        println!("\tMAC: {}", mac);
        println!("\tIPs: {:?}", interface.ips);
    }

    // <http server>
    let mut mount = Mount::new();
    mount.mount("/", Static::new(Path::new("public")));
    thread::spawn(|| { Iron::new(mount).http("[::]:3000").unwrap(); });
	// </http server>

    let iface_name = "wlp2s0";
    let interfaces = datalink::interfaces();
    let interface = interfaces.into_iter().filter(|iface: &NetworkInterface| iface.name == iface_name).next().unwrap();

    let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("packetdump: unhandled channel type: {}"),
        Err(e) => panic!("packetdump: unable to create channel: {}", e),
    };

    let mut iter = rx.iter();
    loop {
        match iter.next() {
            Ok(packet) => handle_packet(&interface.name[..], &packet),
            Err(e) => panic!("packetdump: unable to receive packet: {}", e),
        }
    }
}