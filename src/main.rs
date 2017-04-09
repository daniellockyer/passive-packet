#![feature(rustc_private)]

extern crate pnet;
extern crate iron;
extern crate staticfile;
extern crate mount;
extern crate rustc_serialize;

use rustc_serialize::json;
use std::path::Path;
use std::thread;
use std::sync::Mutex;
use std::sync::Arc;

use iron::{Iron, Request, Response};
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

fn handle_arp_packet(interface_name: &str, ethernet: &EthernetPacket) {
    let header = ArpPacket::new(ethernet.payload());
    if let Some(header) = header {
        println!("[{}]: ARP packet: {}({}) > {}({}); operation: {:?}",
			interface_name, ethernet.get_source(), header.get_sender_proto_addr(),
            ethernet.get_destination(), header.get_target_proto_addr(), header.get_operation());
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
    let interfaces = datalink::interfaces();

    for interface in interfaces.clone() {
        let mac = interface.mac.map(|mac| mac.to_string()).unwrap_or_else(|| "N/A".to_owned());
        println!("{}({}) - {}:", interface.name, interface.index, interface.flags);
        println!("\tMAC: {} -- {:?}", mac, interface.ips);
    }

	let mut data = Arc::new(Mutex::new(Vec::new()));
	let data_closure = data.clone();

    // <http server>
    let mut mount = Mount::new();
	mount
		.mount("/", Static::new(Path::new("public")))
		.mount("/data", move |_: &mut Request| {
			let ref data2 = *data_closure.lock().expect("Unable to lock output");
			Ok(Response::with((iron::status::Ok, json::encode(&data2).unwrap())))
		});
    thread::spawn(|| { Iron::new(mount).http("[::]:3000").unwrap(); });
	// </http server>

    let iface_name = "wlp2s0";
    let interface = interfaces.into_iter().find(|iface: &NetworkInterface| iface.name == iface_name).unwrap();

    let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("packetdump: unhandled channel type"),
        Err(e) => panic!("packetdump: unable to create channel: {}", e),
    };

    let mut iter = rx.iter();
    loop {
    	let mut data = data.lock().expect("Unable to lock output");
    	data.push(iface_name);

        match iter.next() {
            Ok(packet) => match packet.get_ethertype() {
		        EtherTypes::Ipv4 => handle_ipv4_packet(iface_name, &packet),
		        EtherTypes::Ipv6 => handle_ipv6_packet(iface_name, &packet),
		        EtherTypes::Arp => handle_arp_packet(iface_name, &packet),
		        _ => println!("[{}]: Unknown packet: {} > {}; ethertype: {:?} length: {}", iface_name,
		        	&packet.get_source(), &packet.get_destination(), &packet.get_ethertype(), &packet.packet().len())
		    },
            Err(e) => panic!("packetdump: unable to receive packet: {}", e),
        }
    }
}