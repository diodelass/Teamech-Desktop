# Teamech

## Introduction
For many folks who work on technology, the "Internet of Things" has become a scary term. It 
brings to mind completely frivolous and frighteningly insecure systems that let you use your
smartphone to control your household appliances remotely, usually involving a propretary app
and company-hosted web service for each device. In spite of how awful this is, I don't think
that the core concept of networked devices is always useless and silly, and for the few 
particular applications where network control makes sense, it's possible to implement it in
a simple, useful, and sane way. Teamech is my first attempt to do this. It attempts to be a
minimal, easy-to-understand SCADA system for controlling small networks of devices on the 
scale of a household or laboratory, with adequate security and very small resource footprint.
The main embedded device I have in mind is the Raspberry Pi, which has enough computing power
to do a lot of neat things while remaining low-power and inexpensive. A Pi can currently act
as either a server or a client on the network; In the future, versions of the client targeting 
smaller and cheaper microcontroller modules are also planned.

## Network Architecture
Teamech uses a star topology for its networks. Networks must include exactly one server, but
may include any number of clients. Messages sent from one client to the server are relayed to
all other clients. The transport layer is UDP, chosen over TCP to allow greater downtime for 
client devices and keep latency as low as possible. By default, Teamech servers listen and 
transmit on UDP port 6666, but this is configurable. Clients may use any free port.
As UDP is a connectionless protocol, Teamech uses "subscriptions" to manage which packets are
sent where. When a client authenticates itself to the server, the server adds it to a list of
"subscribed" (active) clients, and begins relaying messages from other clients to the new 
client. Clients are unsubscribed when they cancel their subscription or fail to acknowledge a
relayed message.

## Security
Teamech includes its own custom encryption scheme, Teacrypt, which is designed to be simple 
and reasonably secure. While it should not be relied upon in cases where security is critical,
it should be good enough to prevent your nosy neighbors, the local police, or the NSA from
spying on you thanks to its high toughness against brute-force decryption and man-in-the-
middle attacks. Teacrypt provides integrity verification for all messages and requires clients
to authenticate using their encryption keys before they can subscribe; messages that were not
encrypted correctly with the same key that the server uses are rejected and not relayed.
As a symmetric-key algorithm, however, Teacrypt relies on the physical security of both the 
server and the client devices, and so these devices must be trusted and physically accounted 
for at all times for the network to remain secure. Additionally, exchange of keys must be done 
out-of-band before a client can contact a server.

## Server
The Teamech server is essentially a very simple packet relay with message authentication. It
can run on very low-powered hardware, and requires network throughput capability equal to the
maximum continuous throughput from each client times the typical number of clients. For most 
control applications, this throughput will be very low.

