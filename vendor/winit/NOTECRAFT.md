winit 0.30.13, unchanged except `src/platform_impl/android/mod.rs`: a touch carries a force only
when the tool is a stylus or an eraser, so NoteCraft can draw with the pen and not with a finger.
Remove this copy when winit forwards the tool type (then read it in crates/ui-egui/src/stylus.rs).
