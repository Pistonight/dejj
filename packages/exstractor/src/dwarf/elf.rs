use std::sync::Arc;

use cu::pre::*;
use elf::ElfBytes;
use elf::endian::LittleEndian as ElfLittleEndian;
use gimli::{DwarfFileType, EndianSlice, LittleEndian as DwarfLittleEndian};

use crate::dwarf::{In, UnitIter};

/// Holder of Dwarf info, backed by a shared ELF buffer
pub struct Dwarf {
    pub(crate) dwarf: gimli::Dwarf<In<'static>>,
    _buf: ArcBuf,
}

impl Dwarf {
    /// Parse the DWARF in the ELF bytes
    pub fn try_parse(buf: Arc<[u8]>) -> cu::Result<Arc<Self>> {
        let raw_buf = ArcBuf::new(buf);
        // safety: the lifetime of raw_buf_ref is managed
        // by the Arc.
        let raw_buf_ref: &'static [u8] = unsafe { &*raw_buf.0 };
        let elf_data = ElfBytes::<ElfLittleEndian>::minimal_parse(raw_buf_ref);
        let elf_data = cu::check!(elf_data, "failed to parse ELF")?;

        let mut dwarf = gimli::Dwarf::load(|section| {
            let section_name = section.name();
            cu::debug!("loading ELF section {section_name}");
            let header = cu::check!(
                elf_data.section_header_by_name(section_name),
                "cannot read ELF section header for section {section_name}"
            )?;
            let endian_slice = match header {
                Some(header) => {
                    let start = header.sh_offset as usize;
                    let end = start + header.sh_size as usize;
                    cu::debug!("found ELF section {section_name} at byte start=0x{start:016x}, end=0x{end:016x}");
                    EndianSlice::new(&raw_buf_ref[start..end], DwarfLittleEndian)
                }
                None => {
                    cu::debug!("did not found ELF section {section_name}");
                    EndianSlice::new(&[], DwarfLittleEndian)
                }
            };
            cu::Ok(endian_slice)
        })
        .context("failed to load DWARF from ELF")?;
        dwarf.file_type = DwarfFileType::Main;

        Ok(Arc::new(Self {
            dwarf,
            _buf: raw_buf,
        }))
    }

    pub fn iter_units(self_: &Arc<Self>) -> UnitIter {
        let iter = self_.dwarf.debug_info.units();
        UnitIter {
            debug_info_iter: iter,
            dwarf: Arc::clone(&self_),
        }
    }
}

struct ArcBuf(*const [u8]);
impl ArcBuf {
    fn new(buf: Arc<[u8]>) -> Self {
        Self(Arc::into_raw(buf))
    }
}
impl Drop for ArcBuf {
    fn drop(&mut self) {
        unsafe {
            Arc::from_raw(self.0);
        }
    }
}
unsafe impl Send for ArcBuf {}
unsafe impl Sync for ArcBuf {}
