extern crate rustc_serialize;
extern crate pnet;
extern crate iron;
extern crate staticfile;
extern crate mount;

use std::{env,thread,process};
use std::path::Path;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::{Mutex, Arc};

use rustc_serialize::json::{ToJson, Json};
use iron::{Iron, Request, Response};
use staticfile::Static;
use mount::Mount;

use pnet::packet::Packet;
use pnet::packet::ethernet::{EtherTypes, EtherType, EthernetPacket};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::arp::ArpPacket;
use pnet::datalink::{self, NetworkInterface};
use pnet::datalink::Channel::Ethernet;

struct Communication {
	src: String,
	dst: String,
	typ: EtherType,
	value: u32,
	local: bool,
}

impl Communication {
	fn new(src: String, dst: String, typ: EtherType, value: u32, local: bool) -> Communication {
		Communication {
			src: src,
			dst: dst,
			typ: typ, //Unused, just need to fill gap.
			value: value,
			local: local
		}
	}
}

impl ToJson for Communication {
	fn to_json(&self) -> Json {
		let mut d = BTreeMap::new();
		d.insert("src".to_string(), self.src.to_json());
		d.insert("dst".to_string(), self.dst.to_json());
		d.insert("type".to_string(), format!("{}", self.typ).to_json());
		d.insert("value".to_string(), self.value.to_json());
		d.insert("local".to_string(), self.local.to_json());
		Json::Object(d)
	}
}

struct CommStore {
	data: Vec<Communication>,
}

impl ToJson for CommStore {
	fn to_json(&self) -> Json {
		self.data.to_json()
	}
}

impl CommStore {
	fn new() -> CommStore {
		let mut ip_list = Vec::new();

		for interface in datalink::interfaces() {
			if let Some(ips) = interface.ips {
				for ip in ips {
					ip_list.push(Communication::new(format!("{}", ip), format!("{}", ip), EtherType(0x0000), 0, true));
				}
			}
		}

		CommStore {
			data: ip_list
		}
	}

	fn add(&mut self, src: String, dst: String, packet: EthernetPacket) {
		for e in &mut self.data {
			if e.src == src && e.dst == dst && e.typ == packet.get_ethertype() {
				e.value += 1;
				return;
			}
		}

		self.data.push(Communication::new(src, dst, packet.get_ethertype(), 1, false));
	}
}

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

	let data = Arc::new(Mutex::new(CommStore::new()));
	let data_closure = data.clone();
	let mut mount = Mount::new();

	mount.mount("/", Static::new(Path::new("public"))).mount("/data", move |_: &mut Request| {
		let data2 = &(*data_closure.lock().expect("Unable to lock output"));
		let json_data = data2.to_json();
		Ok(Response::with((iron::status::Ok, json_data.to_string())))
	});

	thread::spawn(|| Iron::new(mount).http("[::]:3000").unwrap());
	println!("Listening on http://[::]:3000");

	let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
		Ok(Ethernet(tx, rx)) => (tx, rx),
		Ok(_) => panic!("[!] Unhandled channel type"),
		Err(e) => panic!("[!] Unable to create channel: {}", e),
	};

	let mut iter = rx.iter();
	loop {
		match iter.next() {
			Ok(packet) => {
				let mut data = data.lock().expect("Unable to lock output");

				let (src, dst): (String, String) = match packet.get_ethertype() {
					EtherTypes::Ipv4 => {
						let header = Ipv4Packet::new(packet.payload());
						if let Some(header2) = header {
							(header2.get_source().to_string(), header2.get_destination().to_string())
						} else {
							("N/A".to_string(), "N/A".to_string())
						}
					},
					EtherTypes::Ipv6 => {
						let header = Ipv6Packet::new(packet.payload());
						if let Some(header2) = header {
							(header2.get_source().to_string(), header2.get_destination().to_string())
						} else {
							("N/A".to_string(), "N/A".to_string())
						}
					},
					EtherTypes::Arp => {
						let header = ArpPacket::new(packet.payload());
						if let Some(header2) = header {
							(header2.get_sender_proto_addr().to_string(), header2.get_target_proto_addr().to_string())
						} else {
							("N/A".to_string(), "N/A".to_string())
						}
					},
					_ => ("N/A".to_string(), "N/A".to_string())
				};
				data.add(src, dst, packet);
			},
			Err(e) => panic!("[!] Unable to receive packet: {}", e),
		}
	}
}
