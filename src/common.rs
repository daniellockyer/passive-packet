extern crate pnet;

use self::pnet::datalink;

#[derive(Debug, RustcDecodable, RustcEncodable)]
pub struct Communication {
	pub src: String,
	pub src_group: String,
	pub dst: String,
	pub dst_group: String,
	pub typ: Vec<String>,
	pub value: u32,
}

impl Communication {
	pub fn new(src: String, src_group: String, dst: String, dst_group: String, typ: Vec<String>, value: u32) -> Communication {
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

#[derive(Debug, RustcDecodable, RustcEncodable)]
pub struct CommStore {
	pub data: Vec<Communication>,
}

impl CommStore {
	pub fn new() -> CommStore {
		let mut ip_list = Vec::new();

		for interface in datalink::interfaces() {
			for ip in interface.ips {
				ip_list.push(Communication::new(ip.ip().to_string(), "private".to_string(), ip.ip().to_string(),
					"private".to_string(), vec!(), 0));
			}
		}

		CommStore {
			data: ip_list
		}
	}

	pub fn add(&mut self, comm: Communication) {
		for e in &mut self.data {
			for t in &comm.typ {
				if e.src == comm.src && e.dst == comm.dst {
					if !e.typ.contains(&t) {
						e.typ.push(t.clone());
					}

					e.value += 1;
					return;
				}
			}
		}

		self.data.push(comm);
	}
}