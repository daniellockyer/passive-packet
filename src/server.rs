extern crate rustc_serialize;
extern crate iron;
extern crate staticfile;
extern crate mount;

mod common;

use std::io::Read;
use std::sync::{Arc, Mutex};
use rustc_serialize::json;
use iron::{Iron, Request, Response};
use mount::Mount;

fn main() {
	let data = Arc::new(Mutex::new(common::CommStore::new()));
    let data_clone = data.clone();
	let mut mount = Mount::new();

	mount
		.mount("/", staticfile::Static::new(std::path::Path::new("public")))
		.mount("/data", move |_: &mut Request| {
			let data2 = &(*data_clone.lock().expect("Unable to lock output"));
			let json_data = json::encode(data2).unwrap();
			Ok(Response::with((iron::status::Ok, json_data.to_string())))
		})
		.mount("/new", move |req: &mut Request| {
			let mut payload = String::new();
        	req.body.read_to_string(&mut payload).unwrap();

			let decoded: common::CommStore = json::decode(&payload).unwrap();
			let mut curr_store = data.lock().expect("Unable to lock output");

			for c in decoded.data {
				curr_store.add(c);
			}

			Ok(Response::with((iron::status::Ok, "{}".to_string())))
		});

	println!("Listening on http://[::]:3000");
	Iron::new(mount).http("[::]:3000").unwrap();
}
