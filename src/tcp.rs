use std::io;

enum State {
    // Listen,
    SynRcvd,
    Estab,
}

pub struct Connection {
    state: State,
    send: SendSequenceSpace,
    recv: RecvSequenceSpace,
    ip: etherparse::Ipv4Header,
}

/// State of the Send Sequence Space (RFC 793 S3.2 F4)
///
/// ```
///           1         2          3          4
///      ----------|----------|----------|----------
///             SND.UNA    SND.NXT    SND.UNA
///                                  +SND.WND
///
/// 1 - old sequence numbers which have been acknowledged
/// 2 - sequence numbers of unacknowledged data
/// 3 - sequence numbers allowed for new data transmission
/// 4 - future sequence numbers which are not yet allowed
/// ```
struct SendSequenceSpace {
    /// send unacknowledged
    una: u32,
    /// send next
    nxt: u32,
    /// send window
    wnd: u16,
    /// send urgent pointer
    up: bool,
    /// segment sequence number for last window update
    wl1: usize,
    /// segment acknowledgement number used for last window update
    wl2: usize,
    /// initial send sequence number
    iss: u32,
}

/// State of the Receive Sequence Space (RFC 793 S3.2 F5)
///
/// ```
///               1          2          3
///           ----------|----------|----------
///                  RCV.NXT    RCV.NXT
///                            +RCV.WND
///
/// 1 - old sequence numbers which have been acknowledged
/// 2 - sequence numbers allowed for new reception
/// 3 - future sequence numbers which are not yet allowed
/// ```
struct RecvSequenceSpace {
    /// receive next
    nxt: u32,
    /// receive window
    wnd: u16,
    /// receive urgent pointer
    up: bool,
    /// initial receive sequence number
    irs: u32,
}

impl Connection {
    pub fn accept<'a>(
        nic: &mut tun_tap::Iface,
        iph: etherparse::Ipv4HeaderSlice<'a>,
        tcph: etherparse::TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) -> io::Result<Option<Self>> {
        let mut buf = [0u8; 1500];
        if !tcph.syn() {
            // only expected SYN packet
            return Ok(None);
        }
        
        let iss = 0;
        let mut c = Connection {
            state: State::SynRcvd,
            send: SendSequenceSpace {
                iss,
                una: iss,
                nxt: iss + 1,
                wnd: 10,
                up: false,

                wl1: 0,
                wl2: 0,

            },
            recv: RecvSequenceSpace {
                irs: tcph.sequence_number(),
                nxt: tcph.sequence_number() + 1,
                wnd: tcph.window_size(),
                up: false,
            },
            ip: etherparse::Ipv4Header::new(
                0,
                64, 
                etherparse::IpTrafficClass::Tcp,
                [
                    iph.destination()[0],
                    iph.destination()[1],
                    iph.destination()[2],
                    iph.destination()[3],
                ],
                [
                    iph.source()[0],
                    iph.source()[1],
                    iph.source()[2],
                    iph.source()[3],
                ],
            ),
        };

        // need to start establishing a connection
        let mut syn_ack = 
            etherparse::TcpHeader::new(
                tcph.destination_port(),
                tcph.source_port(),
                c.send.iss,
                c.send.wnd,
            );
        syn_ack.acknowledgment_number = c.recv.nxt;
        syn_ack.syn = true;
        syn_ack.ack = true;
        c.ip.set_payload_len(syn_ack.header_len() as usize + 0);
        // the kernel is nice and does this for us
        // syn_ack.checksum = syn_ack
        //     .calc_checksum_ipv4(&ip, &[])
        //     .expect("failed to compute checksum");

        // write out the headers
        let unwritten = {
            let mut unwritten = &mut buf[..];
            c.ip.write(&mut unwritten);
            syn_ack.write(&mut unwritten);
            unwritten.len()
        };
        nic.send(&buf[..unwritten])?;
        Ok(Some(c))
    }

    pub fn on_packet<'a>(
        &mut self,
        nic: &mut tun_tap::Iface,
        iph: etherparse::Ipv4HeaderSlice<'a>,
        tcph: etherparse::TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) -> io::Result<()> {
        // acceptable ack check
        // SND.UNA < SEG.ACK =< SND.NXT
        // but remember wrapping!
        let ackn = tcph.acknowledgment_number();
        if self.send.una < ackn {
            // check is violated iff n is between u and a
            if self.send.nxt >= self.send.una && self.send.nxt < ackn {
                return Ok(());
            }
        } else {
            // check is okay iff n is between u and a
            if self.send.nxt >= ackn && self.send.nxt < self.send.una {
            } else {
                return Ok(());
            }
        }

        match self.state {
            State::SynRcvd => {
                // expect to get an ACK for our SYN
            }
            State::Estab => {
                unimplemented!();
            }
        }

        Ok(())
    }
}
