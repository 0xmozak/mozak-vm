use std::ptr::addr_of;

use once_cell::unsync::Lazy;
use rkyv::ser::serializers::{AllocScratch, CompositeSerializer, HeapScratch};
use rkyv::{Archive, Deserialize, Serialize};

use crate::coretypes::{CPCMessage, CanonicalEvent, Event, Poseidon2HashType, ProgramIdentifier};

pub type RkyvSerializer = rkyv::ser::serializers::AlignedSerializer<rkyv::AlignedVec>;
pub type RkyvScratch = rkyv::ser::serializers::FallbackScratch<HeapScratch<256>, AllocScratch>;
pub type RkyvShared = rkyv::ser::serializers::SharedSerializeMap;

pub trait RkyvSerializable =
    rkyv::Serialize<CompositeSerializer<RkyvSerializer, RkyvScratch, RkyvShared>>;
pub trait CallArgument = Sized + RkyvSerializable;
pub trait CallReturn = ?Sized + Clone + Default + RkyvSerializable + Archive;

#[derive(Default, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Archive, Serialize, Deserialize))]
#[cfg_attr(not(target_os = "mozakvm"), archive_attr(derive(Debug)))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct SystemTapes {
    pub private_tape: RawTape,
    pub public_tape: RawTape,
    pub call_tape: CallTape,
    pub event_tape: EventTape,
}

#[cfg(target_os = "mozakvm")]
extern "C" {
    static _mozak_self_prog_id: usize;
    static _mozak_cast_list: usize;
    static _mozak_public_io_tape: usize;
    static _mozak_private_io_tape: usize;
    static _mozak_call_tape: usize;
    static _mozak_event_tape: usize;
}

static mut SYSTEM_TAPES: Lazy<SystemTapes> = Lazy::new(|| {
    #[cfg(target_os = "mozakvm")]
    #[allow(clippy::cast_ptr_alignment)]
    #[allow(clippy::ptr_as_ptr)]
    {
        use std::ptr::slice_from_raw_parts;

        /// Zero-copy deserialization on a memory region starting at `addr`
        /// Expected layout to be `[<data_region len (N) in 4
        /// bytes>|<data_region N bytes>]`
        fn get_zcd_repr<T: rkyv::Archive>(addr: *const u8) -> &'static <T as Archive>::Archived {
            let mem_len = unsafe { *{ addr as *const u32 } } as usize;
            unsafe {
                let mem_slice = &*slice_from_raw_parts::<u8>(addr.add(4), mem_len);
                rkyv::archived_root::<T>(mem_slice)
            }
        }

        let self_prog_id =
            unsafe { *{ addr_of!(_mozak_self_prog_id) as *const ProgramIdentifier } };
        assert_ne!(self_prog_id, ProgramIdentifier::default()); // Reserved for null caller

        let castlist_zcd = get_zcd_repr::<Vec<ProgramIdentifier>>(unsafe {
            addr_of!(_mozak_cast_list) as *const u8
        });
        let cast_list: Vec<ProgramIdentifier> =
            castlist_zcd.deserialize(&mut rkyv::Infallible).unwrap();

        let calltape_zcd =
            get_zcd_repr::<Vec<CPCMessage>>(unsafe { addr_of!(_mozak_call_tape) as *const u8 });

        let eventtape_zcd =
            get_zcd_repr::<Vec<Event>>(unsafe { addr_of!(_mozak_event_tape) as *const u8 });

        SystemTapes {
            call_tape: CallTape {
                cast_list,
                self_prog_id,
                reader: Some(calltape_zcd),
                index: 0,
            },
            event_tape: EventTape {
                self_prog_id,
                reader: Some(eventtape_zcd),
                index: 0,
            },
            ..SystemTapes::default()
        }
    }

    #[cfg(not(target_os = "mozakvm"))]
    {
        // let calls = vec![CPCMessage::default(), CPCMessage::default()];
        SystemTapes::default()
    }
});

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct RawTape {
    start: u32,
    len: u32,
}

#[derive(Default, Clone)]
#[cfg(target_os = "mozakvm")]
pub struct CallTape {
    cast_list: Vec<ProgramIdentifier>,
    self_prog_id: ProgramIdentifier,
    reader: Option<&'static <Vec<CPCMessage> as Archive>::Archived>,
    index: usize,
}

#[cfg(not(target_os = "mozakvm"))]
#[derive(Default, Debug, Clone, Archive, Deserialize, Serialize)]
#[archive_attr(derive(Debug))]
pub struct CallTape {
    pub writer: Vec<CPCMessage>,
}

impl CallTape {
    #[cfg(target_os = "mozakvm")]
    /// Check if a given actor takes part in this `CallTape`'s cast list.
    fn is_casted_actor(&self, actor: &ProgramIdentifier) -> bool {
        &ProgramIdentifier::default() == actor || self.cast_list.contains(actor)
    }

    #[allow(clippy::similar_names)]
    pub fn from_mailbox(&mut self) -> Option<(CPCMessage, usize)> {
        #[cfg(target_os = "mozakvm")]
        {
            while self.index < self.reader.unwrap().len() {
                let zcd_cpcmsg = &self.reader.unwrap()[self.index];
                let caller: ProgramIdentifier = zcd_cpcmsg
                    .caller_prog
                    .deserialize(&mut rkyv::Infallible)
                    .unwrap();

                assert_ne!(caller, self.self_prog_id);

                let callee: ProgramIdentifier = zcd_cpcmsg
                    .callee_prog
                    .deserialize(&mut rkyv::Infallible)
                    .unwrap();

                assert!(self.is_casted_actor(&caller));

                // if we are the callee, return this message
                if self.self_prog_id == callee {
                    let full_deserialized: CPCMessage =
                        zcd_cpcmsg.deserialize(&mut rkyv::Infallible).unwrap();
                    self.index += 1;
                    return Some((full_deserialized, self.index - 1));
                }
                self.index += 1;
            }
            None
        }

        #[cfg(not(target_os = "mozakvm"))]
        {
            // TODO(bing): implement native from_mailbox()
            return None;
        }
    }

    #[allow(clippy::similar_names)]
    #[allow(unused_variables)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn to_mailbox<A, R>(
        &mut self,
        caller_prog: ProgramIdentifier,
        callee_prog: ProgramIdentifier,
        call_args: A,
        dispatch_native: impl Fn(A) -> R,
        _dispatch_mozakvm: impl Fn() -> R,
    ) -> R
    where
        A: CallArgument + PartialEq,
        R: CallReturn,
        <A as Archive>::Archived: Deserialize<A, rkyv::Infallible>,
        <R as Archive>::Archived: Deserialize<R, rkyv::Infallible>, {
        #[cfg(target_os = "mozakvm")]
        {
            assert!(self.index < self.reader.unwrap().len());

            let zcd_cpcmsg = &self.reader.unwrap()[self.index];
            let cpcmsg: CPCMessage = zcd_cpcmsg.deserialize(&mut rkyv::Infallible).unwrap();

            assert_eq!(cpcmsg.caller_prog, self.self_prog_id);
            assert_eq!(cpcmsg.callee_prog, callee_prog);
            assert!(self.is_casted_actor(&callee_prog));

            let zcd_args = unsafe { rkyv::archived_root::<A>(&cpcmsg.args.0[..]) };
            let deserialized_args = <<A as Archive>::Archived as Deserialize<
                A,
                rkyv::Infallible,
            >>::deserialize(zcd_args, &mut rkyv::Infallible)
            .unwrap();

            assert!(deserialized_args == call_args);

            self.index += 1;

            let zcd_ret = unsafe { rkyv::archived_root::<R>(&cpcmsg.ret.0[..]) };
            <<R as Archive>::Archived as Deserialize<R, rkyv::Infallible>>::deserialize(
                zcd_ret,
                &mut rkyv::Infallible,
            )
            .unwrap()
        }
        #[cfg(not(target_os = "mozakvm"))]
        {
            let msg = CPCMessage {
                caller_prog,
                callee_prog,
                args: rkyv::to_bytes::<_, 256>(&call_args).unwrap().into(),
                ..CPCMessage::default()
            };

            self.writer.push(msg);
            let inserted_idx = self.writer.len() - 1;

            let retval = dispatch_native(call_args);

            self.writer[inserted_idx].ret = rkyv::to_bytes::<_, 256>(&retval).unwrap().into();

            println!(
                "[CALL ] ResolvedAdd: {:#?}",
                self.writer[inserted_idx].clone()
            );

            retval
        }
    }
}

#[derive(Default, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Archive, Deserialize, Serialize))]
#[cfg_attr(not(target_os = "mozakvm"), archive_attr(derive(Debug)))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct EventTape {
    #[cfg(target_os = "mozakvm")]
    self_prog_id: ProgramIdentifier,
    #[cfg(target_os = "mozakvm")]
    reader: Option<&'static <Vec<Event> as Archive>::Archived>,
    #[cfg(not(target_os = "mozakvm"))]
    pub writer: Vec<EventTapeSingle>,
    #[cfg(target_os = "mozakvm")]
    index: usize,
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct EventTapeSingle {
    pub id: ProgramIdentifier,
    pub contents: Vec<Event>,
    pub canonical_repr: Option<CanonicalEventTapeSingle>,
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct CanonicalEventTapeSingle {
    /// sorted according to address, and opcode.
    pub contents: Vec<CanonicalEvent>,
}

impl From<EventTapeSingle> for CanonicalEventTapeSingle {
    fn from(value: EventTapeSingle) -> Self {
        #[cfg(target_os = "mozakvm")]
        {
            unimplemented!()
        }

        #[cfg(not(target_os = "mozakvm"))]
        {
            let mut event_tape = value
                .contents
                .iter()
                .map(|event| CanonicalEvent::from(event.clone()))
                .collect::<Vec<CanonicalEvent>>();
            event_tape.sort_by(|a, b| {
                a.address
                    .cmp(&b.address)
                    .then(a.event_type.cmp(&b.event_type))
            });
            Self {
                contents: event_tape,
            }
        }
    }
}

impl EventTape {
    #[must_use]
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "mozakvm")]
            self_prog_id: ProgramIdentifier::default(),
            #[cfg(target_os = "mozakvm")]
            reader: None,
            #[cfg(not(target_os = "mozakvm"))]
            writer: vec![],
            #[cfg(target_os = "mozakvm")]
            index: 0,
        }
    }

    #[allow(unused_variables)]
    pub fn emit_event(&mut self, id: ProgramIdentifier, event: Event) {
        #[cfg(target_os = "mozakvm")]
        {
            use crate::coretypes::CanonicalEventType;
            assert!(self.index < self.reader.unwrap().len());

            let zcd_event = &self.reader.unwrap()[self.index];
            let event_deserialized: Event = zcd_event.deserialize(&mut rkyv::Infallible).unwrap();

            assert_eq!(event, event_deserialized);

            assert_eq!(
                match event.operation {
                    CanonicalEventType::Create
                    | CanonicalEventType::Delete
                    | CanonicalEventType::Write => event.object.constraint_owner,
                    _ => self.self_prog_id,
                },
                self.self_prog_id
            );

            self.index += 1;
        }
        #[cfg(not(target_os = "mozakvm"))]
        {
            println!("[EVENT] Add: {:#?}", event);

            // TODO: Sad code, fix later
            for single_tape in self.writer.iter_mut() {
                if single_tape.id == id {
                    single_tape.contents.push(event);
                    return;
                }
            }
            self.writer.push(EventTapeSingle {
                id,
                contents: vec![event],
                canonical_repr: None,
            });
        }
    }
}

#[cfg(not(target_os = "mozakvm"))]
pub fn dump_tapes(file_template: String) {
    fn write_to_file(file_path: &String, content: &[u8]) {
        use std::io::Write;
        let path = std::path::Path::new(file_path.as_str());
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content).unwrap();
    }

    let mut tape_clone = unsafe { SYSTEM_TAPES.clone() }; // .clone() removes `Lazy{}`
    tape_clone.event_tape.writer.iter_mut().for_each(|event| {
        event.canonical_repr = Some(CanonicalEventTapeSingle::from(event.clone()))
    });

    let dbg_filename = file_template.clone() + ".tape_debug";
    let dbg_bytes = &format!("{:#?}", tape_clone).into_bytes();
    println!("[TPDMP] Debug  dump: {:?}", dbg_filename);
    write_to_file(&dbg_filename, dbg_bytes);

    let bin_filename = file_template + ".tape_bin";
    let bin_bytes = unsafe { rkyv::to_bytes::<_, 256>(&*(addr_of!(tape_clone))).unwrap() };
    println!("[TPDMP] Binary dump: {:?}", bin_filename);
    write_to_file(&bin_filename, bin_bytes.as_slice());
}

/// ---- SDK accessible methods ---
pub enum IOTape {
    Private,
    Public,
}

/// Emit an event from `mozak_vm` to provide receipts of
/// `reads` and state updates including `create` and `delete`.
/// Panics on event-tape non-abidance.
pub fn event_emit(id: ProgramIdentifier, event: Event) {
    unsafe { SYSTEM_TAPES.event_tape.emit_event(id, event) }
}

/// Receive one message from mailbox targetted to us and its index
/// "consume" such message. Subsequent reads will never
/// return the same message. Panics on call-tape non-abidance.
#[must_use]
pub fn call_receive() -> Option<(CPCMessage, usize)> {
    unsafe { SYSTEM_TAPES.call_tape.from_mailbox() }
}

/// Send one message from mailbox targetted to some third-party
/// resulting in such messages finding itself in their mailbox
/// Panics on call-tape non-abidance.
#[allow(clippy::similar_names)]
pub fn call_send<A, R>(
    caller_prog: ProgramIdentifier,
    callee_prog: ProgramIdentifier,
    call_args: A,
    dispatch_native: impl Fn(A) -> R,
    dispatch_mozakvm: impl Fn() -> R,
) -> R
where
    A: CallArgument + PartialEq,
    R: CallReturn,
    <A as Archive>::Archived: Deserialize<A, rkyv::Infallible>,
    <R as Archive>::Archived: Deserialize<R, rkyv::Infallible>, {
    unsafe {
        SYSTEM_TAPES.call_tape.to_mailbox(
            caller_prog,
            callee_prog,
            call_args,
            dispatch_native,
            dispatch_mozakvm,
        )
    }
}

/// Get raw pointer to access iotape (unsafe) without copy into
/// buffer. Subsequent calls will provide pointers `num` away
/// (consumed) from pointer provided in this call for best
/// effort safety. `io_read` and `io_read_into` would also affect
/// subsequent returns.
/// Unsafe return values, use wisely!!
#[must_use]
pub fn io_raw_read(_from: &IOTape, _num: usize) -> *const u8 { unimplemented!() }

/// Get a buffer filled with num elements from choice of `IOTape`
/// in process "consuming" such bytes.
#[must_use]
pub fn io_read(_from: &IOTape, _num: usize) -> Vec<u8> { unimplemented!() }

/// Fills a provided buffer with num elements from choice of `IOTape`
/// in process "consuming" such bytes.
pub fn io_read_into(_from: &IOTape, _buf: &mut [u8]) { unimplemented!() }

#[must_use]
pub fn poseidon2_hash(input: &[u8]) -> Poseidon2HashType {
    #[cfg(target_os = "mozakvm")]
    {
        pub const RATE: usize = 8;
        use mozak_system::system::syscall_poseidon2;

        use crate::coretypes::DIGEST_BYTES;

        // VM expects input length to be multiple of RATE
        assert!(input.len() % RATE == 0);
        let mut output = [0; DIGEST_BYTES];
        syscall_poseidon2(input.as_ptr(), input.len(), output.as_mut_ptr());
        Poseidon2HashType(output)
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        use plonky2::field::goldilocks_field::GoldilocksField;
        use plonky2::field::types::Field;
        use plonky2::hash::hashing::PlonkyPermutation;
        use plonky2::hash::poseidon2::{Poseidon2Hash, Poseidon2Permutation};
        use plonky2::plonk::config::{GenericHashOut, Hasher};
        let data_fields: Vec<GoldilocksField> = input
            .iter()
            .map(|x| GoldilocksField::from_canonical_u8(*x))
            .collect();

        const RATE: usize =
            <Poseidon2Permutation<GoldilocksField> as PlonkyPermutation<GoldilocksField>>::RATE;
        assert!(input.len() % RATE == 0);

        Poseidon2HashType(
            Poseidon2Hash::hash_no_pad(&data_fields)
                .to_bytes()
                .try_into()
                .expect("Output length does not match to DIGEST_BYTES"),
        )
    }
}
