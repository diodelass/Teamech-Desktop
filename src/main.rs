/* Teamech Desktop Client v0.6
 * September 2018
 * License: AGPL v3.0
 *
 * This source code is provided with ABSOLUTELY NO WARRANTY. You are fully responsible for any
 * operations that your computers carry out as a result of running this code or anything derived
 * from it. The developer assumes the full absolution of liability described in the AGPL v3.0
 * license.
 * 
 * OVERVIEW
 * Teamech is a simple, low-bandwidth supervisory control and data relay system intended for
 * internet-connected household appliances. Both clients and servers maintain security using a
 * strong encryption protocol, Teacrypt, for message secrecy and integrity verification. While 
 * this protocol is thought to be secure, neither the specification nor this implementation have
 * been formally verified, and as such should not be relied upon in life-or-death or otherwise
 * high-stakes situations.
 * Teamech is suitable for small-scale household use. As the server routes all packets to all
 * nodes, it does not scale well to very large systems, and is best deployed as a multi-drop
 * command delivery system to allow a single user client, such as this one, to control a small
 * cluster of controller clients attached to the hardware being managed.
 * This file contains the source code for the Teamech client, which expects to communicate with the
 * Teamech server. The distribution in which you received this file should also contain the source
 * code for the server.
 *
 *
 * COMMAND-LINE FLAGS
 *
 * --showhex / -h 
 *	  Prints the hex values of all sent and received characters after the lossy-utf8 string
 *	  version in the console. Useful if dealing in messages which are binary and not
 *	  human-readable.
 *

Cargo.toml:
[package]
name = "teamech-console"
version = "0.6.0"
authors = ["ellie"]

[dependencies]
tiny-keccak = "1.4.2"
rand = "0.3"
pancurses = "0.16"
dirs = "1.0.3"
chrono = "0.4"
byteorder = "1"

 */

static VERSION:&str = "0.6 September 2018";
static MSG_VALID_TIME:i64 = 10_000; // Tolerance interval in ms for packet timestamps outside of which to mark them as suspicious
static LOG_DIRECTORY:&str = ".teamech-logs/desktop";
static PROMPT:&str = "[teamech]: ";

extern crate rand;

extern crate tiny_keccak;
use tiny_keccak::Keccak;

extern crate pancurses;
use pancurses::*;

extern crate dirs;
use dirs::home_dir;

extern crate chrono;
use chrono::prelude::*;

extern crate byteorder;
use byteorder::{LittleEndian,ReadBytesExt,WriteBytesExt};

#[macro_use]
extern crate clap;

use std::env::set_var;
use std::time::Duration;
use std::process;
use std::thread::sleep;
use std::error::Error;
use std::io;
use std::io::prelude::*;
use std::net::{UdpSocket,SocketAddr,ToSocketAddrs};
use std::fs;
use std::fs::File;
use std::path::{Path,PathBuf};

fn i64_bytes(number:&i64) -> [u8;8] {
	let mut bytes:[u8;8] = [0;8];
	match bytes.as_mut().write_i64::<LittleEndian>(*number) {
		Err(why) => {
			println!("FATAL: Could not convert integer to little-endian bytes: {}",why.description());
			process::exit(1);
		},
		Ok(_) => (),
	};
	return bytes;
}

fn u64_bytes(number:&u64) -> [u8;8] {
	let mut bytes:[u8;8] = [0;8];
	match bytes.as_mut().write_u64::<LittleEndian>(*number) {
		Err(why) => {
			println!("FATAL: Could not convert integer to little-endian bytes: {}",why.description());
			process::exit(1);
		},
		Ok(_) => (),
	};
	return bytes;
}

fn bytes_i64(bytes:&[u8;8]) -> i64 {
	return match bytes.as_ref().read_i64::<LittleEndian>() {
		Err(why) => {
			println!("FATAL: Could not convert little-endian bytes to integer: {}",why.description());
			process::exit(1);
		},
		Ok(n) => n,
	};
}

fn bytes_u64(bytes:&[u8;8]) -> u64 {
	return match bytes.as_ref().read_u64::<LittleEndian>() {
		Err(why) => {
			println!("FATAL: Could not convert little-endian bytes to integer: {}",why.description());
			process::exit(1);
		},
		Ok(n) => n,
	};
}

// bytes2hex converts a vector of bytes into a hexadecimal string. This is used mainly for
// debugging, when printing a binary string.
fn bytes2hex(v:&Vec<u8>) -> String {
	let mut result:String = String::from("");
	for x in 0..v.len() {
		if v[x] == 0x00 {
			result.push_str(&format!("00"));
		} else if v[x] < 0x10 {
			result.push_str(&format!("0{:x?}",v[x]));
		} else {
			result.push_str(&format!("{:x?}",v[x]));
		}
		if x < v.len()-1 {
			result.push_str(" ");
		}
	}
	return result;
}

// prints a line to the ncurses window - useful for condensing this common and lengthy invocation elsewhere.
fn windowprint(window:&Window,line:&str) {
	window.mv(window.get_cur_y(),0);
	window.clrtoeol();
	window.addstr(&line);
	window.mv(0,0);
	window.insdelln(-1);
	window.mv(window.get_max_y()-1,0);
	window.clrtoeol();
	window.addstr(&PROMPT);
	window.refresh();
}

// prints a line to the ncurses window and also logs it.
fn windowlog(window:&Window,logfile:&Path,line:&str) {
	log(&window,&logfile,&line);
	window.mv(window.get_cur_y(),0);
	window.clrtoeol();
	window.addstr(&format!("[{}] {}",Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),&line));
	window.mv(0,0);
	window.insdelln(-1);
	window.mv(window.get_max_y()-1,0);
	window.clrtoeol();
	window.addstr(&PROMPT);
	window.refresh();
}

// Accepts a path to a log file, and writes a line to it, generating a human- and machine-readable log.
fn logtofile(logfilename:&Path,logstring:&str,timestamp:DateTime<Local>) -> Result<(),io::Error> {
	let userhome:PathBuf = match home_dir() {
		None => PathBuf::new(),
		Some(pathbuf) => pathbuf,
	};
	let logdir:&Path = &userhome.as_path().join(&LOG_DIRECTORY);
	match fs::create_dir_all(&logdir) {
		Err(why) => return Err(why),
		Ok(_) => (),
	};
	let logpath:&Path = &logdir.join(&logfilename);
	let mut logfile = match fs::OpenOptions::new() 
										.append(true)
										.open(&logpath) {
		Ok(file) => file,
		Err(why) => match why.kind() {
			io::ErrorKind::NotFound => match File::create(&logpath) {
				Ok(file) => file,
				Err(why) => return Err(why),
			},
			_ => return Err(why),
		},
	};
	match writeln!(logfile,"[{}][{}] {}",timestamp.timestamp_millis(),timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),&logstring) {
		Ok(_) => return Ok(()),
		Err(why) => return Err(why),
	};
}

// Error-handling wrapper for logtofile() - rather than returning an error, prints the error
// message to the console and returns nothing.
// invocation template: log(&window,&
fn log(window:&Window,logfilename:&Path,logstring:&str) {
	let timestamp:DateTime<Local> = Local::now();
	match logtofile(&logfilename,&logstring,timestamp) {
		Err(why) => {
			windowprint(&window,&format!("ERROR: Failed to write to log file at {}: {}",logfilename.display(),why.description()));
		},
		Ok(()) => (),
	};
}

// Generates a single-use encryption key from a provided key size, pad file and authentication 
// nonce, and returns the key and its associated secret seed.
fn keygen(nonce:&[u8;8],pad:&Vec<u8>,keysize:&usize) -> (Vec<u8>,Vec<u8>) {
	// Finding the pad size this way won't work if the pad is a block device instead of a regular
	// file. If using the otherwise-valid strategy of using a filesystemless flash device as a pad,
	// this block will need to be extended to use a different method of detecting the pad size.
	let mut seed:[u8;8] = [0;8];
	let mut seednonce:[u8;8] = nonce.clone();
	let mut newseednonce:[u8;8] = [0;8];
	// Hash the nonce, previous hash, and previous byte retrieved eight times, using each hash to 
	// index one byte from the pad file. These eight bytes are the secret seed.
	// The hash is *truncated* to the first eight bytes (64 bits), then *moduloed* to the length of
	// the pad file. (If you try to decrypt by just moduloing the whole hash against the pad
	// length, it won't work.)
	for x in 0..8 {
		let mut sha3 = Keccak::new_sha3_256();
		sha3.update(&nonce.clone());
		sha3.update(&seednonce);
		if x >= 1 {
			sha3.update(&[seed[x-1]]);
		}
		sha3.finalize(&mut newseednonce);
		seednonce = newseednonce;
		seed[x] = pad[bytes_u64(&seednonce) as usize%pad.len()];
	}
	let mut keybytes:Vec<u8> = Vec::with_capacity(*keysize);
	let mut keynonce:[u8;8] = seed;
	let mut newkeynonce:[u8;8] = [0;8];
	// Hash the seed, previous hash, and previous byte retrieved n times, where n is the length of
	// the key to be generated. Use each hash to index bytes from the pad file (with the same
	// method as before). These bytes are the key.
	for x in 0..*keysize {
		let mut sha3 = Keccak::new_sha3_256();
		sha3.update(&seed);
		sha3.update(&keynonce);
		if x >= 1 {
			sha3.update(&[keybytes[x-1]]);
		}
		sha3.finalize(&mut newkeynonce);
		keynonce = newkeynonce;
		keybytes.push(pad[bytes_u64(&keynonce) as usize%pad.len()]);
	}
	return (keybytes,seed.to_vec());
}

// Depends on keygen function; generates a random nonce, produces a key, signs the message using
// the secret seed, and returns the resulting encrypted payload (including the message,
// signature, and nonce).
fn encrypt(message:&Vec<u8>,pad:&Vec<u8>) -> Vec<u8> {
	let nonce:u64 = rand::random::<u64>();
	let noncebytes:[u8;8] = u64_bytes(&nonce);
	let keysize:usize = message.len()+8;
	// Use the keygen function to create a key of length n + 8, where n is the length of the
	// message to be encrypted. (The extra eight bytes are for encrypting the signature.)
	let (keybytes,seed) = keygen(&noncebytes,&pad,&keysize);
	let mut signature:[u8;8] = [0;8];
	let mut sha3 = Keccak::new_sha3_256();
	// Generate the signature by hashing the secret seed, the unencrypted message, and the key used
	// to encrypt the signature and message. 
	sha3.update(&seed);
	sha3.update(&message);
	sha3.update(&keybytes);
	sha3.finalize(&mut signature);
	let mut verimessage = Vec::new();
	verimessage.append(&mut message.clone());
	verimessage.append(&mut signature.to_vec());
	let mut payload = Vec::new();
	for x in 0..keysize {
		payload.push(verimessage[x] ^ keybytes[x]);
	}
	payload.append(&mut noncebytes.to_vec());
	return payload;
}

// Depends on keygen function; uses the nonce attached to the payload to generate the same key and
// secret seed, decrypt the payload, and verify the resulting message with its signature. The
// signature will only validate if the message was the original one encrypted with the same pad 
// file as the one used to decrypt it; if it has been tampered with, generated with a different
// pad, or is just random junk data, the validity check will fail and this function will return an
// io::ErrorKind::InvalidData error.
fn decrypt(payload:&Vec<u8>,pad:&Vec<u8>) -> (bool,Vec<u8>) {
	let mut noncebytes:[u8;8] = [0;8];
	// Detach the nonce from the payload, and use it to generate the key and secret seed.
	noncebytes.copy_from_slice(&payload[payload.len()-8..payload.len()]);
	let keysize = payload.len()-8;
	let ciphertext:Vec<u8> = payload[0..payload.len()-8].to_vec();
	let (keybytes,seed) = keygen(&noncebytes,&pad,&keysize);
	let mut verimessage = Vec::new();
	// Decrypt the message and signature using the key.
	for x in 0..keysize {
		verimessage.push(ciphertext[x] ^ keybytes[x]);
	}
	let mut signature:[u8;8] = [0;8];
	// Detach the signature from the decrypted message, and use it to verify the integrity of the
	// message. If the check succeeds, return Ok() containing the message content; if it fails,
	// return an io::ErrorKind::InvalidData error.
	signature.copy_from_slice(&verimessage[verimessage.len()-8..verimessage.len()]);
	let message:Vec<u8> = verimessage[0..verimessage.len()-8].to_vec();
	let mut truesig:[u8;8] = [0;8];
	let mut sha3 = Keccak::new_sha3_256();
	sha3.update(&seed);
	sha3.update(&message);
	sha3.update(&keybytes);
	sha3.finalize(&mut truesig);
	return (signature == truesig,message);
}

// Sends a vector of bytes to a specific host over a specific socket, automatically retrying in the event of certain errors
// and aborting in the event of others.
fn sendraw(listener:&UdpSocket,destaddr:&SocketAddr,payload:&Vec<u8>) -> Result<(),io::Error> {
	// loop until either the send completes or an unignorable error occurs.
	loop {
		match listener.send_to(&payload[..],destaddr) {
			Ok(nsend) => match nsend < payload.len() {
				// If the message sends in its entirety, exit with success. If it sends
				// incompletely, try again.
				false => return Ok(()),
				true => (),
			},
			Err(why) => match why.kind() {
				// Interrupted just means we need to try again.
				// WouldBlock for a send operation usually means that the transmit buffer is full.
				io::ErrorKind::Interrupted => (),
				io::ErrorKind::WouldBlock => {
					return Err(why);
				},
				_ => {
					return Err(why);
				},
			},
		};
	}
}

// Automatically encrypts a vector of bytes and sends them over the socket.
fn sendbytes(listener:&UdpSocket,destaddr:&SocketAddr,bytes:&Vec<u8>,pad:&Vec<u8>) -> Result<(),io::Error> {
	let mut stampedbytes = bytes.clone();
	stampedbytes.append(&mut i64_bytes(&Local::now().timestamp_millis()).to_vec());
	let payload = encrypt(&stampedbytes,&pad); 
	return sendraw(&listener,&destaddr,&payload);
}

fn main() {
	let arguments = clap_app!(app =>
		(name: "Teamech Console")
		(version: VERSION)
		(author: "Ellie D.")
		(about: "Desktop console client for the Teamech protocol.")
		(@arg ADDRESS: +required "Remote address to contact.")
		(@arg PADFILE: +required "Pad file to use for encryption/decryption (must be same as server's).")
		(@arg name: -n --name +takes_value "Unique identifier to present to the server for routing.")
		(@arg class: -c --class +takes_value "Non-unique identifier to present to the server for routing.")
		(@arg localport: -p --localport +takes_value "UDP port to bind to locally (automatic if unset).")
		(@arg showhex: -h --showhex "Show hexadecimal values of messages (useful if working with binary messages).")
	).get_matches();
	let port:u16 = match arguments.value_of("localport").unwrap_or("0").parse::<u16>() {
		Err(_) => {
			println!("Warning: Could not parse specified local port number as a 16-bit integer. Using auto-selection instead.");
			0
		},
		Ok(n) => n,
	};
	let serverhosts:Vec<SocketAddr> = match arguments.value_of("ADDRESS") {
		None => {
			println!("Usage: teamech-desktop [address:port] [padfile]");
			process::exit(1);
		},
		Some(value) => match value.to_socket_addrs() {
			Err(_) => {
				// Failure to parse a remote address is always a fatal error - if this doesn't work, we
				// have nothing to do.
				println!("Could not parse argument #1 as an IP address or hostname.");
				process::exit(1);
			},
			Ok(addrs) => addrs.collect(),
		},
	};
	let serverhost:SocketAddr = serverhosts[0];
	let padfilename:&str = match arguments.value_of("PADFILE") {
		None => {
			println!("Usage: teamech-desktop [address:port] [padfile]");
			process::exit(1);
		},
		Some(filename) => filename,
	};
	let padpath:&Path = Path::new(&padfilename);
	let clientname:&str = arguments.value_of("name").unwrap_or("human");
	let clientclass:&str = arguments.value_of("class").unwrap_or("supervisor");
	'recovery:loop {
		// Recovery and operator loop structure is similar to that used in the server; the operator
		// loop runs constantly while the program is active, while the recovery loop catches breaks
		// from the operator and smoothly restarts the program in the event of a problem.
		// ncurses machinery (for the fancy console display stuff)
		set_var("ESCDELAY","0"); // force ESCDELAY to be 0, so we can quit the application with the ESC key without the default 1-second delay.
		let window = initscr();
		window.refresh(); // must be called every time the screen is to be updated.
		window.keypad(true); // keypad mode, which is typical 
		window.nodelay(true); // nodelay mode, which ensures that the window is actually updated on time
		noecho(); // prevent local echo, since we'll be handling that ourselves
		window.mv(window.get_max_y()-1,0); // go to the bottom left corner
		window.refresh();
		// Print welcome messages
		windowprint(&window,&format!("Teamech Console {}",&VERSION));
		windowprint(&window,"Press <Esc> to exit (or Ctrl-C to force exit).");
		windowprint(&window,"");
		let logfilename:String = format!("{}-teamech-desktop.log",Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
		let logfile:&Path = Path::new(&logfilename);
		windowprint(&window,&format!("Using log file {} in {}.",&logfilename,&LOG_DIRECTORY));
		windowprint(&window,"");
		match logtofile(&logfile,&format!("Opened log file."),Local::now()) {
			Err(why) => {
				windowprint(&window,&format!("-!- WARNING: Could not open log file at {} - {}. Logs are currently NOT BEING SAVED - you should fix this!",
																												logfile.display(),why.description()));
			},
			Ok(_) => (),
		};
		windowlog(&window,&logfile,&format!("- Loading key data from pad file at {}...",&padpath.display()));
		let mut pad:Vec<u8> = Vec::new();
		match File::open(&padpath) {
			Err(why) => {
				endwin();
				println!("Could not open pad file at {} - {}.",&padpath.display(),&why.description());
				process::exit(1);
			},
			Ok(mut padfile) => match padfile.read_to_end(&mut pad) {
				Err(why) => {
					endwin();
					println!("Could not load key data from pad file at {} - {}.",&padpath.display(),&why.description());
					process::exit(1);
				},
				Ok(_) => {
					windowlog(&window,&logfile,"- Finished loading key data.");
				},
			},
		};
		let listener:UdpSocket = match UdpSocket::bind(&format!("0.0.0.0:{}",port)) {
			Ok(socket) => socket,
			Err(why) =>	{
				// Error condition: bind to local address failed. This is probably caused by a
				// network issue, a transient OS issue (e.g. network permissions/firewall), or
				// another program (or another instance of this one) occupying the port the user 
				// specified. In any case, we can't continue, so we'll let the user know what the
				// problem is and quit.
				endwin();
				println!("Could not bind to local address: {}.",why.description());
				process::exit(1);
			},
		};
		match listener.set_nonblocking(true) {
			Ok(_) => (),
			Err(why) => {
				// This is probably a platform error - it's not clear to me when this would happen,
				// but it probably means that the OS doesn't support nonblocking UDP sockets, which
				// is weird and means this program won't really work. Hopefully, the error message
				// will be useful to the user.
				endwin();
				println!("Could not set socket to nonblocking mode: {}.",why.description());
				process::exit(1);
			},
		}
		// Set up some system state machinery
		let mut inbin:[u8;500] = [0;500]; // input buffer for receiving bytes
		let mut lastmsgs:Vec<Vec<u8>> = Vec::new(); // keeps track of messages that have already been received, to merge double-sends.
		let mut consoleline:Vec<u8> = Vec::new(); // the bytestring holding the text currently typed into the console line editor
		let mut linehistory:Vec<Vec<u8>> = Vec::new(); // the history of text entered at the console, for up-arrow message repeating
		let mut ackbuffer:Vec<Vec<u8>> = Vec::new(); // Stores lines whose transmission has not been confirmed.
		let mut historypos:usize = 0; // the scroll position of the history list, for up-arrow message repeating
		let mut linepos:usize = 0; // the position of the cursor in the console line
		'authtry:loop {
			windowlog(&window,&logfile,&format!("- Trying to contact server..."));
			match sendbytes(&listener,&serverhost,&vec![],&pad) {
				Err(why) => {
					windowlog(&window,&logfile,&format!("-!- Could not send authentication payload - {}.",why.description()));
					sleep(Duration::new(5,0));
					continue 'authtry;
				},
				Ok(_) => (),
			};
			for _ in 0..10 {
				sleep(Duration::new(0,100_000_000));
				match listener.recv_from(&mut inbin) {
					Err(why) => match why.kind() {
						io::ErrorKind::WouldBlock => (),
						_ => {
							windowlog(&window,&logfile,&format!("-!- Could not receive authentication response - {}.",why.description()));
							sleep(Duration::new(5,0));
							continue 'authtry;
						},
					},
					Ok((nrecv,srcaddr)) => {
						if nrecv == 25 && srcaddr == serverhost {
							let (datavalid,message):(bool,Vec<u8>) = decrypt(&inbin[0..25].to_vec(),&pad);
							if datavalid {
								match message[0] {
									0x02 => {
										windowlog(&window,&logfile,&format!("- Subscribed to server at {}.",serverhost));
										let mut namemsg:Vec<u8> = vec![0x01];
										let mut classmsg:Vec<u8> = vec![0x11];
										namemsg.append(&mut clientname.as_bytes().to_vec());
										classmsg.append(&mut clientclass.as_bytes().to_vec());
										let _ = sendbytes(&listener,&serverhost,&namemsg,&pad);
										let _ = sendbytes(&listener,&serverhost,&classmsg,&pad);
										break 'authtry;
									},
									0x19 => {
										windowlog(&window,&logfile,
														&format!("-!- Pad file is correct, but subscription rejected by server. Server may be full."));
										sleep(Duration::new(5,0));
									},
									other => {
										windowlog(&window,&logfile,&format!("-!- Server at {} sent an unknown status code {}. Are these versions compatible?",
																															serverhost,other));
										sleep(Duration::new(5,0));
									},
								};
							} else {
								windowlog(&window,&logfile,&format!("-!- Response from server did not validate. Local pad file is incorrect or invalid."));
							}
						} else {
							windowlog(&window,&logfile,&format!("-!- Got invalid message of length {} from {}.",nrecv,srcaddr));
							sleep(Duration::new(5,0));
						}
					}, // recv Ok
				}; // match recv
			} // for 0..10
		} // 'authtry
		// Yay! If we made it down here, that means we're successfully authenticated and
		// subscribed, and can start doing the things this program is actually meant for.
		'operator:loop {
			sleep(Duration::new(0,1_000_000));
			'receiver:loop {
				match listener.recv_from(&mut inbin) {
					Err(why) => match why.kind() {
						io::ErrorKind::WouldBlock => break 'receiver,
						_ => {
							// Receive error
							windowlog(&window,&logfile,&format!("-!- Could not receive packet: {}. Trying again in 5 seconds...",why.description()));
							sleep(Duration::new(5,0));
						},
					},
					Ok((nrecv,srcaddr)) => {
						if srcaddr != serverhost {
							continue 'operator;
						}
						if nrecv > 24 {
							if lastmsgs.contains(&inbin[0..nrecv].to_vec()) {
								// Ignore the payload if it's a duplicate. This will never
								// false-positive, because even repeated messages will be encrypted
								// with different keys and generate different payloads. Repeated
								// payloads are always messages that were double-sent or replayed,
								// and not the client deliberately sending the same thing again.
								continue 'operator;
							} else {
								lastmsgs.push(inbin[0..nrecv].to_vec());
								if lastmsgs.len() > 32 {
									lastmsgs.reverse();
									let _ = lastmsgs.pop();
									lastmsgs.reverse();
								}
							}
							let payload:Vec<u8> = inbin[0..nrecv].to_vec();
							let (datavalid,message):(bool,Vec<u8>) = decrypt(&payload,&pad);
							// If a message is successfully received and decrypted, display
							// it. Also indicate if the message timestamp was invalid
							// (either too far in the future or too far in the past), but
							// since this is a human-facing client, we just want to flag
							// these messages, not hide them completely.
							let messagebytes:Vec<u8> = message[0..message.len()-8].to_vec();
							let mut messagetext:String = String::from_utf8_lossy(&messagebytes).to_string();
							let messageparts:Vec<&str> = messagetext.splitn(2," ").collect::<Vec<&str>>();
							let mut messagesender:&str = "<unspecified>";
							let mut messagecontents:&str = "";
							if messageparts.len() == 2 {
								messagesender = messageparts[0];
								messagecontents = messageparts[1];
							} else if messageparts.len() == 1 {
								messagecontents = messageparts[0];
							}
							let messagechars:Vec<u8> = messagecontents.as_bytes().to_vec();
							let mut timestamp:[u8;8] = [0;8];
							timestamp.copy_from_slice(&message[message.len()-8..message.len()]);
							let msgtime:i64 = bytes_i64(&timestamp);
							let mut msgstatus:String = String::new();
							if !datavalid {
								msgstatus = format!(" [INVALID SIGNATURE]");
							} else if msgtime + MSG_VALID_TIME < Local::now().timestamp_millis() {
								msgstatus = format!(" [OUTDATED]");
							} else if msgtime - MSG_VALID_TIME > Local::now().timestamp_millis() {
								msgstatus = format!(" [FUTURE]");
							}
							match (msgstatus.as_ref(),message[0],messagebytes.len()) {
								("",0x19,1) => { // END OF MEDIUM
									// Handle deauthentications
									windowlog(&window,&logfile,&format!("- Subscription expiration notification received - renewing subscription to {}...",
																																serverhost));
									continue 'recovery;
								},
								("",0x02,1) => {
									// Handle requests for identification
									windowlog(&window,&logfile,&format!("- Subscribed to server at {}.",serverhost));
									let mut namemsg:Vec<u8> = vec![0x01];
									let mut classmsg:Vec<u8> = vec![0x11];
									namemsg.append(&mut clientname.as_bytes().to_vec());
									classmsg.append(&mut clientclass.as_bytes().to_vec());
									let _ = sendbytes(&listener,&serverhost,&namemsg,&pad);
									let _ = sendbytes(&listener,&serverhost,&classmsg,&pad);
									if ackbuffer.len() > 0 {
										let _ = sendbytes(&listener,&srcaddr,&ackbuffer[0],&pad);
									}
								},
								("",0x06,1) => (),
								("",0x06,3) => {
									if ackbuffer.len() > 0 {
										ackbuffer.remove(0);
									}
									let mut nsends:u16 = 0;
									nsends += message[2] as u16;
									nsends += (message[1] as u16) << 8;
									let nsendstr:&str = &format!("[ {} ]",&nsends.to_string());
									if window.get_cur_y() > 0 {
										// Display response codes from the server on the right-hand side of
										// the terminal, on the same line as the outgoing message the
										// response corresponds to.
										// This is a special case, NOT something that should be
										// replaced with a simple call to windowlog().
										window.mv(window.get_cur_y()-1,window.get_max_x()-(nsendstr.len() as i32));
										window.clrtoeol();
										window.addstr(format!("{}",&nsendstr));
										window.mv(window.get_cur_y()+1,0);
										window.clrtoeol();
										window.addstr(&PROMPT);
										window.addstr(&format!("{}",String::from_utf8_lossy(&consoleline)));
										window.refresh();
									}
								},
								("",0x05,_) => {
									windowlog(&window,&logfile,&format!("- {}",&String::from_utf8_lossy(&message[1..message.len()-8])));
								},
								(status,_,n) => {
									if arguments.is_present("showhex") {
										windowlog(&window,&logfile,&format!("<{}>{}: {} [{}] [{}]",
												messagesender,status,messagecontents,bytes2hex(&messagechars),n));
									} else {
										windowlog(&window,&logfile,&format!("<{}>{}: {}",messagesender,status,messagecontents));
									}
									let _ = sendbytes(&listener,&srcaddr,&vec![0x06],&pad);
								},
							};
						} // if nrecv > 24
					}, // recv ok
				}; // match recvfrom
			} // 'receiver
			match window.getch() { 
				// This is where we process keypress events. Most of these are going to be related
				// to implementing basic line editing.
				Some(Input::Character(c)) => match c as u8 {
					0x0A => { // ENTER
						// This means "send the message", so we start by printing it to the screen
						// above the input line.
						if arguments.is_present("showhex") {
							windowlog(&window,&logfile,&format!("<local>: {} [{}]",String::from_utf8_lossy(&consoleline),bytes2hex(&consoleline)));
						} else {
							windowlog(&window,&logfile,&format!("<local>: {}",String::from_utf8_lossy(&consoleline)));
						}
						historypos = 0;
						if linehistory.len() == 0 || linehistory[linehistory.len()-1] != consoleline {
							// Append this line to the history, provided the last message sent
							// isn't identical to this one.
							linehistory.push(consoleline.clone());
						}
						// Send (and encrypt) the message.
						match sendbytes(&listener,&serverhost,&consoleline,&pad) {
							Err(why) => {
								windowlog(&window,&logfile,&format!("-!- Sending message failed - {}",why.description()));
								continue 'operator;
							},
							Ok(_) => (),
						};
						ackbuffer.push(consoleline.clone());
						consoleline = Vec::new();
						linepos = 0;
					},
					0x7F|0x08 => { // DEL
						// Handles both backspace and delete the same way, by knocking out the
						// character just before the cursor position.
						if linepos > 0 {
							let _ = consoleline.remove(linepos-1); 
							window.mv(window.get_cur_y(),window.get_cur_x()-1);
							window.delch();
							linepos -= 1;
							window.refresh();
						}
					},
					0x1B => { // ESCAPE
						let _ = sendbytes(&listener,&serverhost,&vec![0x18],&pad);
						endwin();
						process::exit(0);
					}
					c => {
						// This means the key was an actual character that needs to be added to the
						// console line, as opposed to a special key for controlling the editor.
						if linepos == consoleline.len() {
							window.addch(c as char);
							consoleline.push(c);
						} else {
							window.insch(c as char);
							consoleline.insert(linepos,c);
							window.mv(window.get_cur_y(),window.get_cur_x()+1);
						}
						linepos += 1;
						window.refresh();
					},
				},
				Some(Input::KeyBackspace) => {
					// Maybe something's screwed up with ASCII del being sent, so we get this
					// instead of the char 0x7F? Handle it the same way as a fallback.
					if linepos > 0 {
						let _ = consoleline.remove(linepos-1); 
						window.mv(window.get_cur_y(),window.get_cur_x()-1);
						window.delch();
						linepos -= 1;
						window.refresh();
					}
				}
				Some(Input::KeyUp) => {
					// The user is no doubt accustomed to being able to press the up and down arrow
					// keys to scroll through previously-sent messages, so we implement that here
					// and in the next block.
					if historypos == 0 && consoleline.len() > 0 {
						linehistory.push(consoleline.clone());
					}
					if historypos < linehistory.len() {
						historypos += 1;
						consoleline = linehistory[linehistory.len()-historypos].to_vec();
						window.mv(window.get_cur_y(),0);
						window.clrtoeol();
						window.addstr(&PROMPT);
						window.addstr(&String::from_utf8_lossy(&consoleline));
						linepos = consoleline.len();
						window.refresh();
					}
				},
				Some(Input::KeyDown) => {
					// If we're at the bottom of the history already, pressing the down arrow
					// should clear the input line (essentially, we're pretending that there's
					// always an empty message at the bottom of the history).
					if historypos > 1 {
						historypos -= 1;
						consoleline = linehistory[linehistory.len()-historypos].to_vec();
					} else if consoleline.len() > 0 {
						if historypos == 0 {
							linehistory.push(consoleline.clone());
						}
						consoleline = Vec::new();
						historypos = 0;
					}
					window.mv(window.get_cur_y(),0);
					window.clrtoeol();
					window.addstr(&PROMPT);
					window.addstr(&String::from_utf8_lossy(&consoleline));
					linepos = consoleline.len();
					window.refresh();
				},
				Some(Input::KeyLeft) => {
					// left and right arrows do what you'd expect (move left and right in the line
					// editor to select different characters, and also not move off the end of the
					// line)
					if linepos > 0 {
						linepos -= 1;
						if linepos < window.get_max_x() as usize {
							window.mv(window.get_cur_y(),window.get_cur_x()-1);
						}
					}
					window.refresh();
				},
				Some(Input::KeyRight) => {
					if linepos < consoleline.len() as usize {
						linepos += 1;
						if linepos < window.get_max_x() as usize{
							window.mv(window.get_cur_y(),window.get_cur_x()+1);
						}
					}
					window.refresh();
				},
				Some(Input::KeyHome) => {
					// home and end are also straightforward and don't do anything uncommon.
					window.mv(window.get_cur_y(),PROMPT.len() as i32); // for some reason ncurses positions have to be i32 instead of usize? :eyeroll:
					linepos = 0;
					window.refresh();
				},
				Some(Input::KeyEnd) => {
					if PROMPT.len()+linepos >= window.get_max_x() as usize {
						window.mv(window.get_cur_y(),window.get_max_x()-1);
					} else {
						window.mv(window.get_cur_y(),(PROMPT.len()+consoleline.len()) as i32);
					}
					linepos = consoleline.len();
					window.refresh();
				},
				// This stuff is just debugging.
				//Some(Input::KeyResize) => (), 
				//Some(input) => {
				//	window.addstr(&format!("{:?}",input));
				//},
				Some(x) => windowprint(&window,&format!("-!- unknown keypress: {:?}",x)),
				None => (),
			};
			window.refresh();
		} // 'operator
	} // 'recovery
} // fn main

