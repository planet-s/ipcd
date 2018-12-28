use std::{
    cmp,
    collections::{HashMap, hash_map::Entry},
    rc::Rc
};
use syscall::{error::*, Error, SchemeMut, Result};

#[derive(Default)]
pub struct ShmHandle {
    buffer: Option<Box<[u8]>>,
    refs: usize
}
#[derive(Default)]
pub struct ShmScheme {
    maps: HashMap<Rc<str>, ShmHandle>,
    handles: HashMap<usize, Rc<str>>,
    next_id: usize
}

impl SchemeMut for ShmScheme {
    fn open(&mut self, path: &[u8], _flags: usize, _uid: u32, _gid: u32) -> Result<usize> {
        let path = std::str::from_utf8(path).or(Err(Error::new(EPERM)))?.into();
        let entry = self.maps.entry(Rc::clone(&path)).or_insert(ShmHandle::default());
        entry.refs += 1;
        self.handles.insert(self.next_id, path);

        let id = self.next_id;
        self.next_id += 1;
        Ok(id)
    }
    fn fmap(&mut self, id: usize, offset: usize, len: usize) -> Result<usize> {
        let path = self.handles.get(&id).ok_or(Error::new(EBADF))?;
        match self.maps.get_mut(path).expect("handle pointing to nothing").buffer {
            Some(ref mut buf) => {
                if offset + len != buf.len() {
                    return Err(Error::new(ERANGE));
                }
                Ok(buf[offset..].as_mut_ptr() as usize)
            },
            ref mut buf @ None => {
                *buf = Some(vec![0; offset+len].into_boxed_slice());
                Ok(buf.as_mut().unwrap()[offset..].as_mut_ptr() as usize)
            }
        }
    }
    fn fpath(&mut self, id: usize, buf: &mut [u8]) -> Result<usize> {
        // Write scheme name
        const PREFIX: &[u8] = b"shm:";
        let len = cmp::min(PREFIX.len(), buf.len());
        buf[..len].copy_from_slice(&PREFIX[..len]);
        if len < PREFIX.len() {
            return Ok(len);
        }

        // Write path
        let path = self.handles.get(&id).ok_or(Error::new(EBADF))?;
        let len = cmp::min(path.len(), buf.len() - PREFIX.len());
        buf[PREFIX.len()..][..len].copy_from_slice(&path.as_bytes()[..len]);

        Ok(PREFIX.len() + len)
    }
    fn close(&mut self, id: usize) -> Result<usize> {
        let path = self.handles.remove(&id).ok_or(Error::new(EBADF))?;
        let mut entry = match self.maps.entry(path) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => panic!("handle pointing to nothing")
        };
        entry.get_mut().refs -= 1;
        if entry.get().refs == 0 {
            // There is no other reference to this entry, drop
            entry.remove_entry();
        }
        Ok(0)
    }
}
