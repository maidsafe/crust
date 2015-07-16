// Copyright 2015 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.0.  This, along with the
// Licenses can be found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

use time;
use std::fs::File;
use std::io::prelude::*;
use std::path;
use std::env;
use rustc_serialize::json;
use std::io;
use itertools::Itertools;
use config_utils::Contacts;

const MAX_CONTACTS: usize = 1500;

pub struct BootstrapHandler {
    file_name: String,
    last_updated: time::Tm,
}


pub fn serialise_contacts(contacts: Contacts) -> Vec<u8> {
    let encoded = json::encode(&contacts).unwrap();
    return encoded.into_bytes();
}

pub fn parse_contacts(buffer: Vec<u8>) -> Option<Contacts> {
    String::from_utf8(buffer).ok().and_then(|contacts_str| {
        json::decode(&contacts_str).ok()
    })
}



impl BootstrapHandler {
    pub fn get_file_name() -> String {
        let path = match env::current_exe() {
                Ok(exe_path) => exe_path,
                Err(e) => panic!("Failed to get current exe path: {}", e),
            };
        let name_with_extension = path.file_name().expect("Unknown filename");
        let name = path::Path::new(name_with_extension).file_stem()
            .expect("Unknown extension");

        let mut filename = String::new();
        filename.push_str("./");
        filename.push_str(name.to_str().unwrap());
        filename.push_str(".bootstrap.cache");
        filename
    }

    pub fn new() -> BootstrapHandler {
        BootstrapHandler {
            file_name: BootstrapHandler::get_file_name(),
            last_updated: time::now(),
        }
    }

    pub fn get_update_duration() -> time::Duration {
        time::Duration::hours(4)
    }

    pub fn update_contacts(&mut self, contacts: Contacts, prune: Contacts) -> io::Result<()> {
        try!(self.insert_contacts(contacts, prune));
        // TODO(Team) this implementation is missing and should be considered in next planning
        if time::now() > self.last_updated + BootstrapHandler::get_update_duration() {
            // self.check_bootstrap_contacts();
        }
        Ok(())
    }

    pub fn read_bootstrap_file(&self) -> io::Result<(Contacts)> {
        let mut file = try!(File::open(&self.file_name));
        let mut contents = String::new();
        let _ = try!(file.read_to_string(&mut contents));
        json::decode(&contents)
             .map_err(|error| io::Error::new(io::ErrorKind::Other,
                                             format!("Failed to decode bootstrap file: {}", error)))
    }

    #[allow(dead_code)]
    pub fn oldest_contacts(&mut self, n: usize) -> io::Result<(Contacts)> {
        let bootstrap_contacts = self.read_bootstrap_file()
            .unwrap_or_else(|e| {
                println!("Failed to read Bootstrap file : {:?} ; {:?}.", self.file_name, e);
                Contacts::new()
            });
        Ok(bootstrap_contacts.iter().rev().map(|contact| contact.clone())
                             .take(n).collect::<Contacts>())
    }

    fn write_bootstrap_file(&mut self, mut contacts: Contacts) -> io::Result<()> {
        contacts = contacts.clone().into_iter().unique().collect();
        let mut file = try!(File::create(&self.file_name));
        try!(write!(&mut file, "{}", json::as_pretty_json(&contacts)));
        file.sync_all()
    }

    fn insert_contacts(&mut self, mut contacts: Contacts, prune: Contacts) -> io::Result<()> {
        let mut bootstrap_contacts = self.read_bootstrap_file()
            .unwrap_or_else(|e| {
                println!("Failed to read Bootstrap cache file : {:?} ; {:?} ; Creating New file.",
                self.file_name, e);
                Contacts::new()
            });

        if prune.len() > 0 {
            bootstrap_contacts.retain(|x| !prune.contains(&x));
        }

        contacts.retain(|x| !bootstrap_contacts.contains(&x));

        if bootstrap_contacts.len() == 0usize {
            bootstrap_contacts = contacts;
        } else {
            loop {
                if (bootstrap_contacts.len() < MAX_CONTACTS) && (!contacts.is_empty()) {
                    bootstrap_contacts.insert(0usize, contacts.remove(0usize));
                } else {
                    break;
                }
            }
        }

        self.write_bootstrap_file(bootstrap_contacts)
    }

    pub fn get_serialised_contacts(&self) -> io::Result<(Vec<u8>)> {
        let contacts = try!(self.read_bootstrap_file());
        Ok(serialise_contacts(contacts))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{net, fs};
    use std::net::{SocketAddr, Ipv4Addr};
    use transport::Endpoint;
    use rustc_serialize::json;
    use rand;
    use std::path::Path;
    use config_utils::{Contact, Contacts};

    use super::MAX_CONTACTS;

    #[test]
    fn serialisation() {
        let addr = net::SocketAddrV4::new(net::Ipv4Addr::new(1,2,3,4), 8080);
        let contact  = Contact { endpoint: Endpoint::Tcp(SocketAddr::V4(addr)) };
        let mut contacts = Contacts::new();
        contacts.push(contact.clone());
        contacts.push(contact.clone());
        let encoded = json::encode(&contacts).unwrap();
        let decoded: Contacts = json::decode(&encoded).unwrap();
        assert_eq!(contacts, decoded);
    }

    #[test]
    fn bootstrap_handler_test() {
        let mut contacts = Vec::new();
        for _ in 0..10 {
            let mut random_addr_0 = Vec::with_capacity(4);
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());

            let port_0: u16 = rand::random::<u16>();
            let addr_0 = net::SocketAddrV4::new(net::Ipv4Addr::new(random_addr_0[0],
                random_addr_0[1], random_addr_0[2], random_addr_0[3]), port_0);
            let new_contact = Contact { endpoint: Endpoint::Tcp(SocketAddr::V4(addr_0)) };
            contacts.push(new_contact);
        }

        let file_name = BootstrapHandler::get_file_name();
        let path = Path::new(&file_name);

        let mut bootstrap_handler = BootstrapHandler::new();
        let file = fs::File::create(&path);
        assert!(file.is_ok()); // Check whether the database file is created
        // Add Contacts
        assert!(bootstrap_handler.insert_contacts(contacts.clone(), Contacts::new()).is_ok());

        // Add duplicate contacts
        for _ in 1..100 {
            assert!(bootstrap_handler.insert_contacts(contacts.clone(), Contacts::new()).is_ok());
        }

        let read_contacts: Contacts = bootstrap_handler.read_bootstrap_file().unwrap();

        assert_eq!(read_contacts, contacts);

        match fs::remove_file(file_name.clone()) {
            Ok(_) => (),
            Err(e) => println!("Failed to remove {}: {}", file_name, e),
        };
    }

    #[test]
    fn duplicates() {
        let mut contacts = Vec::new();
        let number = 10usize;
        for _ in 0..number {
            let mut random_addr_0 = Vec::with_capacity(4);
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());

            let port_0: u16 = rand::random::<u16>();
            let addr_0 = net::SocketAddrV4::new(net::Ipv4Addr::new(random_addr_0[0],
                random_addr_0[1], random_addr_0[2], random_addr_0[3]), port_0);
            let new_contact = Contact { endpoint: Endpoint::Tcp(SocketAddr::V4(addr_0)) };
            contacts.push(new_contact);
        }

        let file_name = BootstrapHandler::get_file_name();
        let path = Path::new(&file_name);

        let mut bootstrap_handler = BootstrapHandler::new();
        let file = fs::File::create(&path);
        assert!(file.is_ok());

        // add contacts...
        assert!(bootstrap_handler.update_contacts(contacts.clone(), Contacts::new()).is_ok());

        let recovered_contacts = bootstrap_handler.read_bootstrap_file().unwrap();

        assert_eq!(recovered_contacts, contacts);
        assert_eq!(recovered_contacts.len(), number);

        // try duplicating each contact...
        for i in 0..number {
            let mut duplicate_contacts = Vec::new();
            duplicate_contacts.push(contacts[i].clone());
            assert!(bootstrap_handler.update_contacts(duplicate_contacts, Contacts::new()).is_ok());
        }

        let recovered_contacts = bootstrap_handler.read_bootstrap_file().unwrap();

        // bootstrap contacts should remain unaltered...
        assert_eq!(recovered_contacts, contacts);
        assert_eq!(recovered_contacts.len(), number);

        match fs::remove_file(file_name.clone()) {
            Ok(_) => (),
            Err(e) => println!("Failed to remove {}: {}", file_name, e),
        };
    }

    #[test]
    fn prune() {
        let mut contacts = Vec::new();
        let number = 10usize;
        for _ in 0..number {
            let mut random_addr_0 = Vec::with_capacity(4);
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());

            let port_0: u16 = rand::random::<u16>();
            let addr_0 = net::SocketAddrV4::new(net::Ipv4Addr::new(random_addr_0[0],
                random_addr_0[1], random_addr_0[2], random_addr_0[3]), port_0);
            let new_contact = Contact { endpoint: Endpoint::Tcp(SocketAddr::V4(addr_0)) };
            contacts.push(new_contact);
        }

        let file_name = BootstrapHandler::get_file_name();
        let path = Path::new(&file_name);

        let mut bootstrap_handler = BootstrapHandler::new();
        let file = fs::File::create(&path);
        assert!(file.is_ok());

        // add contacts...
        assert!(bootstrap_handler.update_contacts(contacts.clone(), Contacts::new()).is_ok());

        let recovered_contacts = bootstrap_handler.read_bootstrap_file().unwrap();

        assert_eq!(recovered_contacts, contacts);
        assert_eq!(recovered_contacts.len(), number);

        // prune each contact...
        for i in 0..number {
            let mut prune_contacts = Vec::new();
            prune_contacts.push(contacts[i].clone());
            assert!(bootstrap_handler.update_contacts(Contacts::new(), prune_contacts).is_ok());
        }

        let recovered_contacts = bootstrap_handler.read_bootstrap_file().unwrap();

        // bootstrap contacts should be empty...
        assert!(recovered_contacts.is_empty());

        // add the contacts back...
        assert!(bootstrap_handler.update_contacts(contacts.clone(), Contacts::new()).is_ok());

        let recovered_contacts = bootstrap_handler.read_bootstrap_file().unwrap();

        assert_eq!(recovered_contacts, contacts);
        assert_eq!(recovered_contacts.len(), number);

        // create a new contact...
        let mut ip = Vec::with_capacity(4);

        ip.push(rand::random::<u8>());
        ip.push(rand::random::<u8>());
        ip.push(rand::random::<u8>());
        ip.push(rand::random::<u8>());

        let port = rand::random::<u16>();
        let ipport = net::SocketAddrV4::new(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), port);
        let new_contact = Contact{ endpoint: Endpoint::Tcp(SocketAddr::V4(ipport)) };
        let mut new_contacts = Vec::new();
        new_contacts.push(new_contact.clone());

        // get the last contact in the list and prune it from the bootstrap file...
        let prune_contact = recovered_contacts[recovered_contacts.len() - 1].clone();
        let mut prune_contacts = Vec::new();
        prune_contacts.push(prune_contact.clone());

        // add the new contact while pruning the last...
        assert!(bootstrap_handler.update_contacts(new_contacts.clone(), prune_contacts.clone()).is_ok());

        let recovered_contacts = bootstrap_handler.read_bootstrap_file().unwrap();

        // update contact list with expected entries...
        let _ = contacts.remove(number - 1);
        contacts.insert(0usize, new_contact.clone());

        // check the entries...
        assert_eq!(recovered_contacts, contacts);
        assert_eq!(recovered_contacts.len(), number);

        match fs::remove_file(file_name.clone()) {
            Ok(_) => (),
            Err(e) => println!("Failed to remove {}: {}", file_name, e),
        };
    }

    #[test]
    fn oldest() {
        let mut contacts = Vec::new();
        let number = 12usize;
        let twice_number = number * 2;
        let half_number = number / 2;

        for _ in 0..number {
            let mut random_addr_0 = Vec::with_capacity(4);
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());
            random_addr_0.push(rand::random::<u8>());

            let port_0: u16 = rand::random::<u16>();
            let addr_0 = net::SocketAddrV4::new(net::Ipv4Addr::new(random_addr_0[0],
                random_addr_0[1], random_addr_0[2], random_addr_0[3]), port_0);
            let new_contact = Contact { endpoint: Endpoint::Tcp(SocketAddr::V4(addr_0)) };
            contacts.push(new_contact);
        }

        let file_name = BootstrapHandler::get_file_name();
        let path = Path::new(&file_name);

        let mut bootstrap_handler = BootstrapHandler::new();
        let file = fs::File::create(&path);
        assert!(file.is_ok());

        // add contacts...
        assert!(bootstrap_handler.update_contacts(contacts.clone(), Contacts::new()).is_ok());
        // try taking more than existing number...
        let oldest_contacts = bootstrap_handler.oldest_contacts(twice_number).unwrap();
        let reversed_contacts = contacts.iter().rev().map(|e| e.clone())
                                        .take(number).collect::<Contacts>();

        assert_eq!(oldest_contacts, reversed_contacts);
        assert_eq!(oldest_contacts.len(), number);

        let oldest_contacts = bootstrap_handler.oldest_contacts(half_number).unwrap();
        let reversed_contacts = contacts.iter().rev().map(|e| e.clone())
                                        .take(half_number).collect::<Contacts>();

        assert_eq!(oldest_contacts, reversed_contacts);
        assert_eq!(oldest_contacts.len(), half_number);

        match fs::remove_file(file_name.clone()) {
            Ok(_) => (),
            Err(e) => println!("Failed to remove {}: {}", file_name, e),
        };
    }

    #[test]
    fn max_contacts() {
        let mut contacts = Vec::new();
        for _ in 0..MAX_CONTACTS {
            let mut ip = Vec::with_capacity(4);

            ip.push(rand::random::<u8>());
            ip.push(rand::random::<u8>());
            ip.push(rand::random::<u8>());
            ip.push(rand::random::<u8>());

            let port = rand::random::<u16>();
            let ipport = net::SocketAddrV4::new(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), port);
            let contact = Contact{ endpoint: Endpoint::Tcp(SocketAddr::V4(ipport)) };

            contacts.push(contact);
        }

        let file_name = BootstrapHandler::get_file_name();
        let path = Path::new(&file_name);
        let mut bootstrap_handler = BootstrapHandler::new();
        let file = fs::File::create(&path);

        // check that the file got created...
        assert!(file.is_ok());

        // insert contacts...
        assert!(bootstrap_handler.insert_contacts(contacts.clone(), Contacts::new()).is_ok());
        let recovered_contacts = bootstrap_handler.read_bootstrap_file().unwrap();
        // check that the recovered contacts are the same as the originals...
        assert_eq!(recovered_contacts, contacts);
        // check the number of contacts is MAX_CONTACTS...
        assert_eq!(recovered_contacts.len(), MAX_CONTACTS);

        // create a new contact...
        let mut ip = Vec::with_capacity(4);

        ip.push(rand::random::<u8>());
        ip.push(rand::random::<u8>());
        ip.push(rand::random::<u8>());
        ip.push(rand::random::<u8>());

        let port = rand::random::<u16>();
        let ipport = net::SocketAddrV4::new(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), port);
        let new_contact = Contact{ endpoint: Endpoint::Tcp(SocketAddr::V4(ipport)) };
        let mut new_contacts = Vec::new();
        new_contacts.push(new_contact.clone());

        // try inserting without also pruning...
        assert!(bootstrap_handler.insert_contacts(new_contacts.clone(), Contacts::new()).is_ok());
        let recovered_contacts = bootstrap_handler.read_bootstrap_file().unwrap();
        // check that the recovered contacts are the same as the originals...
        assert_eq!(recovered_contacts, contacts);
        // ...and that the number of contacts is still MAX_CONTACTS...
        assert_eq!(recovered_contacts.len(), MAX_CONTACTS);

        // get the last contact in the list and prune it from the bootstrap file...
        let prune_contact = recovered_contacts[recovered_contacts.len() - 1].clone();
        let mut prune_contacts = Vec::new();
        prune_contacts.push(prune_contact.clone());

        // insert the new contact again pruning the last entry...
        assert!(bootstrap_handler.insert_contacts(new_contacts.clone(), prune_contacts.clone()).is_ok());
        let recovered_contacts = bootstrap_handler.read_bootstrap_file().unwrap();

        // check that the recovered contacts are not the same as the originals...
        assert!(recovered_contacts != contacts);
        // ...and that the number of contacts is still MAX_CONTACTS...
        assert_eq!(recovered_contacts.len(), MAX_CONTACTS);
        // check that the pruned contact is not still at the end of the list...
        let last_contact = recovered_contacts[recovered_contacts.len() - 1].clone();
        assert!(last_contact != prune_contact.clone());
        // check that the new contact is at the start of the list...
        let first_contact = recovered_contacts[0].clone();
        assert_eq!(first_contact, new_contact.clone());

        // remove the bootstrap file from disk...
        match fs::remove_file(file_name.clone()) {
            Ok(_) => (),
            Err(e) => println!("Failed to remove {}: {}", file_name, e),
        };
    }
}
