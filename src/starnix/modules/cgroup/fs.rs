// Copyright 2024 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use starnix_core::task::CurrentTask;
use starnix_core::vfs::{
    CacheMode, FileSystem, FileSystemHandle, FileSystemOps, FileSystemOptions, FsNodeHandle,
    FsNodeInfo, FsStr,
};
use starnix_sync::{FileOpsCore, Locked, Unlocked};
use starnix_types::vfs::default_statfs;
use starnix_uapi::auth::FsCred;
use starnix_uapi::errors::Errno;
use starnix_uapi::{errno, mode, statfs, CGROUP2_SUPER_MAGIC, CGROUP_SUPER_MAGIC};

use crate::cgroup::{Cgroup, CgroupOps, CgroupRoot};
use starnix_sync::Mutex;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

use crate::directory::{CgroupDirectory, CgroupDirectoryHandle};

pub struct CgroupV1Fs {
    pub root: Arc<CgroupRoot>,

    /// All directory nodes of the cgroup hierarchy.
    pub hierarchy: Arc<Hierarchy>,
}

impl CgroupV1Fs {
    pub fn new_fs(
        _locked: &mut Locked<'_, Unlocked>,
        current_task: &CurrentTask,
        options: FileSystemOptions,
    ) -> Result<FileSystemHandle, Errno> {
        let weak_kernel = Arc::downgrade(current_task.kernel());
        let root = CgroupRoot::new(weak_kernel);
        let hierarchy = Hierarchy::new(Arc::downgrade(&root));
        let fs = FileSystem::new(
            current_task.kernel(),
            CacheMode::Uncached,
            CgroupV1Fs { hierarchy: hierarchy.clone(), root: root },
            options,
        )?;
        hierarchy.root.create_root_interface_files(current_task, &fs);
        fs.set_root(hierarchy.root.clone());
        Ok(fs)
    }
}
impl FileSystemOps for CgroupV1Fs {
    fn name(&self) -> &'static FsStr {
        b"cgroup".into()
    }
    fn statfs(
        &self,
        _locked: &mut Locked<'_, FileOpsCore>,
        _fs: &FileSystem,
        _current_task: &CurrentTask,
    ) -> Result<statfs, Errno> {
        Ok(default_statfs(CGROUP_SUPER_MAGIC))
    }
}

pub struct CgroupV2Fs {
    pub root: Arc<CgroupRoot>,

    /// All directory nodes of the cgroup hierarchy.
    pub hierarchy: Arc<Hierarchy>,
}

impl CgroupV2Fs {
    pub fn new_fs(
        _locked: &mut Locked<'_, Unlocked>,
        current_task: &CurrentTask,
        options: FileSystemOptions,
    ) -> Result<FileSystemHandle, Errno> {
        let weak_kernel = Arc::downgrade(current_task.kernel());
        let root = CgroupRoot::new(weak_kernel);
        let hierarchy = Hierarchy::new(Arc::downgrade(&root));
        let fs = FileSystem::new(
            current_task.kernel(),
            CacheMode::Uncached,
            CgroupV2Fs { hierarchy: hierarchy.clone(), root: root },
            options,
        )?;
        hierarchy.root.create_root_interface_files(current_task, &fs);
        fs.set_root(hierarchy.root.clone());
        Ok(fs)
    }
}

impl FileSystemOps for CgroupV2Fs {
    fn name(&self) -> &'static FsStr {
        b"cgroup2".into()
    }
    fn statfs(
        &self,
        _locked: &mut Locked<'_, FileOpsCore>,
        _fs: &FileSystem,
        _current_task: &CurrentTask,
    ) -> Result<statfs, Errno> {
        Ok(default_statfs(CGROUP2_SUPER_MAGIC))
    }
}

/// Represents all directory nodes of a cgroup hierarchy.
pub struct Hierarchy {
    root: CgroupDirectoryHandle,

    /// All non-root cgroup directories, keyed by cgroup's ID.
    nodes: Mutex<HashMap<u64, FsNodeHandle>>,
}

impl Hierarchy {
    pub fn new(root_cgroup: Weak<CgroupRoot>) -> Arc<Hierarchy> {
        Arc::new_cyclic(|weak_hierarchy| Self {
            root: CgroupDirectory::new_root(root_cgroup, weak_hierarchy.clone()),
            nodes: Mutex::new(HashMap::new()),
        })
    }

    /// Looks for the corresponding node in the filesystem, errors if not found.
    pub fn get_node(&self, cgroup: &Arc<Cgroup>) -> Result<FsNodeHandle, Errno> {
        let nodes = self.nodes.lock();
        nodes.get(&cgroup.id()).cloned().ok_or_else(|| errno!(ENOENT))
    }

    /// Returns the corresponding nodes for a set of cgroups.
    pub fn get_nodes(&self, cgroups: &Vec<Arc<Cgroup>>) -> Vec<Option<FsNodeHandle>> {
        let nodes = self.nodes.lock();
        cgroups.iter().map(|cgroup| nodes.get(&cgroup.id()).cloned()).collect()
    }

    /// Creates a new `FsNode` for `directory` and stores it in `nodes`.
    pub fn add_node(
        &self,
        cgroup: &Arc<Cgroup>,
        directory: CgroupDirectoryHandle,
        current_task: &CurrentTask,
        fs: &FileSystemHandle,
    ) -> FsNodeHandle {
        let id = cgroup.id();
        let node = fs.create_node(
            current_task,
            directory,
            FsNodeInfo::new_factory(mode!(IFDIR, 0o755), FsCred::root()),
        );
        let mut nodes = self.nodes.lock();
        nodes.insert(id, node.clone());
        node
    }

    /// Removes an entry from `nodes`, errors if not found.
    pub fn remove_node(&self, cgroup: &Arc<Cgroup>) -> Result<FsNodeHandle, Errno> {
        let id = cgroup.id();
        let mut nodes = self.nodes.lock();
        nodes.remove(&id).ok_or_else(|| errno!(ENOENT))
    }
}
