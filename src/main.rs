extern crate rustc_serialize;
extern crate pnet;
extern crate iron;
extern crate staticfile;
extern crate mount;

use std::{env,thread,process};
use std::path::Path;
use std::io::{self, Write};
use std::sync::{Mutex, Arc};

use rustc_serialize::json::ToJson;
use iron::{Iron, Request, Response};
use staticfile::Static;
use mount::Mount;

use pnet::packet::Packet;
use pnet::packet::ethernet::EtherTypes;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::arp::ArpPacket;
use pnet::datalink::{self, NetworkInterface};
use pnet::datalink::Channel::Ethernet;

fn main() {
    let iface_name = env::args().nth(1).unwrap_or_else(|| {
        writeln!(io::stderr(), "[!] Usage: passive-packet <interface>").unwrap();
        process::exit(1);
    });

    let interface = datalink::interfaces().into_iter()
    					.find(|iface: &NetworkInterface| iface.name == iface_name)
    					.unwrap_or_else(|| {
    						writeln!(io::stderr(), "[!] That interface does not exist.").unwrap();
        					process::exit(1);
    					});

	let data = Arc::new(Mutex::new(Vec::new()));
	let data_closure = data.clone();
    let mut mount = Mount::new();

	mount.mount("/", Static::new(Path::new("public"))).mount("/data", move |_: &mut Request| {
		let data2 = &(*data_closure.lock().expect("Unable to lock output"));
		let json_data = data2.to_json();
		Ok(Response::with((iron::status::Ok, json_data.to_string())))
	});

    thread::spawn(|| Iron::new(mount).http("[::]:3000").unwrap());
    println!("Listening...");

    let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("[!] Unhandled channel type"),
        Err(e) => panic!("[!] Unable to create channel: {}", e),
    };

    let mut iter = rx.iter();
    let mut i = 0;
    loop {
        match iter.next() {
            Ok(packet) => {
            	i += 1;
            	if i % 100 == 0 {
            		println!("Captured {} packets.", i);
            	}
    			let mut data = data.lock().expect("Unable to lock output");

    			let (src, dst): (String, String) = match packet.get_ethertype() {
			        EtherTypes::Ipv4 => {
			        	let header = Ipv4Packet::new(packet.payload());
    					if let Some(header) = header {
							(header.get_source().to_string(), header.get_destination().to_string())
						} else {
							("N/A".to_string(), "N/A".to_string())
						}
			        },
			        EtherTypes::Ipv6 => {
			        	let header = Ipv6Packet::new(packet.payload());
    					if let Some(header) = header {
							(header.get_source().to_string(), header.get_destination().to_string())
						} else {
							("N/A".to_string(), "N/A".to_string())
						}
					},
			        EtherTypes::Arp => {
			        	let header = ArpPacket::new(packet.payload());
    					if let Some(header) = header {
							(header.get_sender_proto_addr().to_string(), header.get_target_proto_addr().to_string())
						} else {
							("N/A".to_string(), "N/A".to_string())
						}
					},
			        _ => ("N/A".to_string(), "N/A".to_string())
			    };

    			data.push(format!("{{\"src\": {:?}, \"dst\": {:?}, \"type\": \"{:?}\"}}", src, dst, packet.get_ethertype()));
		    },
            Err(e) => panic!("[!] Unable to receive packet: {}", e),
        }
    }
}