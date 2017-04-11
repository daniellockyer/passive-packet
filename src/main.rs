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
	mount
		.mount("/", Static::new(Path::new("public")))
		.mount("/data", move |_: &mut Request| {
			let data2 = &(*data_closure.lock().expect("Unable to lock output"));
			let json_data = data2.to_json();
			Ok(Response::with((iron::status::Ok, json_data.to_string())))
		});
    thread::spawn(|| Iron::new(mount).http("[::]:3000").unwrap());

    let iface_name = "wlp2s0";
    let interface = interfaces.into_iter().find(|iface: &NetworkInterface| iface.name == iface_name).unwrap();

    let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("packetdump: unhandled channel type"),
        Err(e) => panic!("packetdump: unable to create channel: {}", e),
    };

    let mut iter = rx.iter();
    loop {
        match iter.next() {
            Ok(packet) => {
    			let mut data = data.lock().expect("Unable to lock output");
    			data.push(format!("{{\"src\": \"{:?}\", \"dst\": \"{:?}\", \"type\": \"{:?}\"}}",
    				packet.get_source(), packet.get_destination(), packet.get_ethertype()));
		    },
            Err(e) => panic!("packetdump: unable to receive packet: {}", e),
        }
    }
}