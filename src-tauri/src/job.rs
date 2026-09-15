//! Job object di Windows: il processo figlio muore con Aethera, a meno che l'utente non
//! scelga di lasciarlo acceso. Altrove è un guscio vuoto.

use std::process::Child;

#[cfg(windows)]
mod imp {
    use super::Child;
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub struct Job(isize);

    impl Job {
        pub fn kill_on_close() -> Result<Self, String> {
            let h = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(|e| format!("CreateJobObject: {e}"))?;
            let job = Job(h.0 as isize);
            job.set_limits(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE)?;
            Ok(job)
        }

        fn handle(&self) -> HANDLE {
            HANDLE(self.0 as *mut c_void)
        }

        fn set_limits(&self, flags: JOB_OBJECT_LIMIT) -> Result<(), String> {
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = flags;
            unsafe {
                SetInformationJobObject(
                    self.handle(),
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            }
            .map_err(|e| format!("SetInformationJobObject: {e}"))
        }

        pub fn assign(&self, child: &Child) -> Result<(), String> {
            let process = HANDLE(child.as_raw_handle() as *mut c_void);
            unsafe { AssignProcessToJobObject(self.handle(), process) }.map_err(|e| format!("AssignProcessToJobObject: {e}"))
        }

        pub fn terminate(&self) {
            let _ = unsafe { TerminateJobObject(self.handle(), 1) };
        }

        /// Toglie «uccidi alla chiusura»: il motore sopravvive all'uscita di Aethera.
        pub fn release(&self) {
            let _ = self.set_limits(JOB_OBJECT_LIMIT(0));
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            let _ = unsafe { CloseHandle(self.handle()) };
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Child;

    pub struct Job;

    impl Job {
        pub fn kill_on_close() -> Result<Self, String> {
            Ok(Job)
        }
        pub fn assign(&self, _child: &Child) -> Result<(), String> {
            Ok(())
        }
        pub fn terminate(&self) {}
        pub fn release(&self) {}
    }
}

pub use imp::Job;
