// #[cfg(not(target_os = "mozakvm"))]
// pub fn dump_tapes(file_template: String) {
//     fn write_to_file(file_path: &String, content: &[u8]) {
//         use std::io::Write;
//         let path = std::path::Path::new(file_path.as_str());
//         let mut file = std::fs::File::create(&path).unwrap();
//         file.write_all(content).unwrap();
//     }

//     let mut tape_clone = unsafe { SYSTEM_TAPES.clone() }; // .clone() removes
// `Lazy{}`     tape_clone
//         .event_tape
//         .writer
//         .iter_mut()
//         .for_each(|single_event_tape| {
//             let mut canonical_event_tape =
//                 CanonicalEventTapeSingle::from(single_event_tape.clone());
//             canonical_event_tape
//                 .sorted_events
//                 .iter_mut()
//                 .for_each(|canonical_event| canonical_event.event_emitter =
// single_event_tape.id);             single_event_tape.canonical_repr =
// Some(canonical_event_tape);         });

//     let dbg_filename = file_template.clone() + ".tape_debug";
//     let dbg_bytes = &format!("{:#?}", tape_clone).into_bytes();
//     println!("[TPDMP] Debug  dump: {:?}", dbg_filename);
//     write_to_file(&dbg_filename, dbg_bytes);

//     let bin_filename = file_template + ".tape_bin";
//     let bin_bytes = unsafe { rkyv::to_bytes::<_,
// 256>(&*(core::ptr::addr_of!(tape_clone))).unwrap() };     println!("[TPDMP]
// Binary dump: {:?}", bin_filename);     write_to_file(&bin_filename,
// bin_bytes.as_slice()); }
