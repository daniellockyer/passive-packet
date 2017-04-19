#![feature(ip)]

extern crate peel_ip;
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
use std::net::{IpAddr, Ipv4Addr};

use rustc_serialize::json::{ToJson, Json};
use iron::{Iron, Request, Response};
use staticfile::Static;
use mount::Mount;

use peel_ip::prelude::*;
use pnet::packet::Packet;
use pnet::datalink::{self, NetworkInterface};
use pnet::datalink::Channel::Ethernet;

struct Communication {
	src: String,
	src_group: String,
	dst: String,
	dst_group: String,
	typ: Vec<String>,
	value: u32,
}

impl Communication {
	fn new(src: String, src_group: String, dst: String, dst_group: String, typ: Vec<String>, value: u32) -> Communication {
		Communication {
			src: src,
			src_group: src_group,
			dst: dst,
			dst_group: dst_group,
			typ: typ,
			value: value,
		}
	}
}

impl ToJson for Communication {
	fn to_json(&self) -> Json {
		let mut d = BTreeMap::new();
		d.insert("src".to_string(), self.src.to_json());
		d.insert("src_group".to_string(), self.src_group.to_json());
		d.insert("dst".to_string(), self.dst.to_json());
		d.insert("dst_group".to_string(), self.dst_group.to_json());
		d.insert("type".to_string(), self.typ.to_json());
		d.insert("value".to_string(), self.value.to_json());
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
			for ip in interface.ips {
				ip_list.push(Communication::new(format!("{}", ip), "private".to_string(), format!("{}", ip),
					"private".to_string(), vec!("unknown".to_string()), 0));
			}
		}

		CommStore {
			data: ip_list
		}
	}

	fn add(&mut self, src: String, src_group: String, dst: String, dst_group: String, packet_type: String) {
		for e in &mut self.data {
			if e.src == src && e.dst == dst {

				if !e.typ.contains(&packet_type) {
					e.typ.push(packet_type);
				}

				e.value += 1;
				return;
			}
		}

		self.data.push(Communication::new(src, src_group, dst, dst_group, vec!(packet_type), 1));
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

	let mut peel = PeelIp::default();
	let mut iter = rx.iter();
	loop {
		match iter.next() {
			Ok(packet) => {
				let result = peel.traverse(&packet.packet(), vec![]).result;
				let (mut src, mut dst) = (IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
				let (mut src_group, mut dst_group) = ("desktop", "desktop");
				let mut packet_type = "unknown";

				for i in result {
					// Layer 1
					if let Some(_) = i.downcast_ref::<EthernetPacket>() { packet_type = "Ethernet"; }

					// Layer 2
					else if let Some(packet) = i.downcast_ref::<ArpPacket>() {
						packet_type = "Arp";
						src = IpAddr::V4(packet.sender_protocol_address);
						dst = IpAddr::V4(packet.target_protocol_address);
					}

					// Layer 3
					else if let Some(packet) = i.downcast_ref::<Ipv4Packet>() {
						packet_type = "IPv4";
						src = IpAddr::V4(packet.src);
						dst = IpAddr::V4(packet.dst);
					}
					else if let Some(packet) = i.downcast_ref::<Ipv6Packet>() {
						packet_type = "IPv6";
						src = IpAddr::V6(packet.src);
						dst = IpAddr::V6(packet.dst);
					}
					else if let Some(_) = i.downcast_ref::<IcmpPacket>() { packet_type = "ICMP"; }
					else if let Some(_) = i.downcast_ref::<Icmpv6Packet>() { packet_type = "ICMPv6"; }
					else if let Some(_) = i.downcast_ref::<EapolPacket>() { packet_type = "EAPOL"; }

					// Layer 4
					else if let Some(_) = i.downcast_ref::<UdpPacket>() { packet_type = "UDP"; }
					else if let Some(_) = i.downcast_ref::<TcpPacket>() { packet_type = "TCP"; }

					// Layer 7
					else if let Some(_) = i.downcast_ref::<DhcpPacket>() { packet_type = "DHCP"; }
					else if let Some(_) = i.downcast_ref::<DnsPacket>() { packet_type = "DNS"; }
					else if let Some(_) = i.downcast_ref::<HttpPacket>() { packet_type = "HTTP"; }
					else if let Some(_) = i.downcast_ref::<NtpPacket>() { packet_type = "NTP"; }
					else if let Some(_) = i.downcast_ref::<SsdpPacket>() { packet_type = "SSDP"; }
					else if let Some(_) = i.downcast_ref::<TlsPacket>() { packet_type = "TLS"; }

					else { println!("{:?}", packet.packet()); }
				}

				if src.is_multicast() { src_group = "broadcast"; }
				if dst.is_multicast() { dst_group = "broadcast"; }

				if src.is_global() { src_group = "internet"; }
				if dst.is_global() { dst_group = "internet"; }

				if src.is_unspecified() { src_group = "unknown"; }
				if dst.is_unspecified() { dst_group = "unknown"; }

				if src.is_documentation() { src_group = "other"; }
				if dst.is_documentation() { dst_group = "other"; }

				let mut data = data.lock().expect("Unable to lock output");
				data.add(src.to_string(), src_group.to_string(), dst.to_string(), dst_group.to_string(), packet_type.to_string());
			},
			Err(e) => panic!("[!] Unable to receive packet: {}", e),
		}
	}
}
