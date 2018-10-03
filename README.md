# Teamech
## A Simple Application Layer for the Intranet of Things

## Notice: This repository is deprecated in favor of the [Teamech library](https://github.com/diodelass/teamech).

## Overview
See also: main documentation on the 
[Teamech server page](https://github.com/diodelass/Teamech-Server "Teamech Server").  
This is the desktop console client for the Teamech protocol, intended to act as the central command
interface for the network's human administrator. Unlike the embedded client, this client provides 
an ncurses interface somewhat similar to terminal-based IRC clients, allowing you to view messages
being sent via a Teamech server and also send messages of your own.

### Building
To build the Teamech console client, follow these steps:  
1. Install ncurses and its development package for your OS (e.g. libncurses5 and libncurses5-dev 
on Debian).
2. Install an up-to-date stable distribution of Rust (per the Rust website, you can do this on most
Linux distributions by running `curl https://sh.rustup.rs -sSf | sh`).
3. Clone this repository (`git clone https://github.com/diodelass/Teamech-Desktop`) and `cd` into
the main directory (`cd Teamech-Desktop`).
4. Run `cargo build --release`.
5. The binary executable will be written to `Teamech-Desktop/target/release/teamech-console` where
it can be run or copied into a `bin/` directory to install it system-wide.  

### Additional Setup
In order to work, both the Teamech server and client must use a large symmetric key file, referred
to elsewhere as a pad file. In theory, any file will work as a pad file, but for optimal security,
the pad file should be generated using a secure random number generator.  
For optimal security, you should replace the pad file and install a new one on all of the network's 
devices every time the network exchanges a total of about half the pad file's size using that pad.
This is not operationally necessary, and there are currently no known vulnerabilities that would cause
failure to update the pads to allow an attacker to gain access to the system or decrypt its messages,
but by doing this, you ensure that you're at least a moving target should this change.  
Pad files should be large enough to be reasonably sure of including every possible byte at least once.
Practically, they should be as large as you can make them while still reasonably holding and transporting
them using the storage media you have available. A few megabytes is probably reasonable.  
On Linux, you can generate a pad file easily using `dd` and `/dev/urandom`. For instance, to create
a 10-megabyte pad:  
`dd if=/dev/urandom of=teamech-september-2018.pad bs=1M count=10 status=progress`  
You should then copy this pad file to the server and all clients, and select it as the pad file to
use at the command line.  
I make absolutely no guaratees about the security of any Teamech network, no matter what key size 
and key life cycle practices you adhere to. This software is a personal project to familiarize myself
with cryptography, network programming, and version control, and you shouldn't trust it in any context.
You probably shouldn't use it at all, but I can't stop you if you're determined.

### Usage
When the console client connects, it will first attempt to authenticate and subscribe itself to the
server by sending it an empty encrypted message. Once this process completes and the subscription is
confirmed, the console will display a prompt (`[teamech]->`). At this point, you can type messages 
into a simple input line, and press enter to have them encrypted and sent to the server. When the 
server replies with a status code, the code will appear in hex form on the far right end of the 
corresponding line.  
Messages sent by you will be prefixed with `[LOC]` in the console window, while those sent by remote
clients will be prefixed with `[REM]`.  
Once compiled, the desktop console client can be run from the command line like so:  
`./teamech-desktop [server address:port number] [local port number (optional)] [path to pad file]`
If unspecified, the local port number will default to 0, which tells the OS to allocate a port 
dynamically (this is fine for the client, since no one needs to remember which port is being used).
For example, if the client should connect to a Teamech server on port 6666 hosted at example.com,
using a pad file in the current directory called `teamech.pad` and a dynamically-allocated local
port, then the command would be  
`./teamech-desktop example.com:6666 teamech.pad`  
