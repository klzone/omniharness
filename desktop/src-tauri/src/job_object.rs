//! Windows 进程树治理：把引擎子进程挂进 Job Object（OD-SYS-701）。
//!
//! 语义：Job Object 设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 后，宿主只要持有
//! 最后一个 Job 句柄；`EngineChannel` 被 Drop 时句柄关闭，Windows 内核会回收
//! 整棵进程树（引擎及它拉起的 node / DSH 孙进程）。这样退出与强杀路径都不依赖
//! POSIX 进程组——Windows 没有进程组，`current_pgid()` 返回 0。
//!
//! 非 Windows 平台是零大小空实现：不创建句柄、不产生系统调用。

#[cfg(not(windows))]
mod imp {
    /// 非 Windows 占位：不持有任何句柄。
    #[derive(Debug, Default)]
    pub struct JobObject;

    impl JobObject {
        /// 非 Windows 没有 Job Object，原样成功返回。
        pub fn create_for_process(_pid: u32) -> std::io::Result<Self> {
            Ok(Self)
        }

        /// 非 Windows 从不持有 Job 句柄。
        pub fn attached(&self) -> bool {
            false
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::io;

    type Handle = *mut c_void;

    // Win32 常量与结构布局见 jobapi2.h / winnt.h。
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: u32 = 9;
    // AssignProcessToJobObject 文档要求 PROCESS_SET_QUOTA + PROCESS_TERMINATE；
    // 之前只申请 PROCESS_SET_INFORMATION，OpenProcess 返回的句柄权限不足，
    // Windows 会以 ERROR_ACCESS_DENIED（Win32 5）拒绝挂接。
    const PROCESS_SET_QUOTA: u32 = 0x0100;
    const PROCESS_SET_INFORMATION: u32 = 0x0200;
    const PROCESS_TERMINATE: u32 = 0x0001;
    const FALSE: i32 = 0;

    #[repr(C)]
    #[derive(Default)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct JobObjectBasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct JobObjectExtendedLimitInformation {
        basic: JobObjectBasicLimitInformation,
        io_info: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attributes: *mut c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            info_class: u32,
            info: *const c_void,
            info_len: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn GetLastError() -> u32;
    }

    fn win32_error(operation: &str) -> io::Error {
        // SAFETY: GetLastError 只读取当前线程的错误码，无参数、无副作用。
        let code = unsafe { GetLastError() };
        io::Error::other(format!("{} 失败（Win32 {}）", operation, code))
    }

    /// 一个设置了 kill-on-close 的 Job Object 句柄。
    ///
    /// 句柄是内核对象引用，可跨线程移动；析构即在宿主线程关闭句柄。
    pub struct JobObject {
        handle: Handle,
    }

    unsafe impl Send for JobObject {}
    unsafe impl Sync for JobObject {}

    impl JobObject {
        /// 创建 Job Object、设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`，并把
        /// 已 spawn 的子进程挂进去。失败时返回错误，调用方不得在无 Job 保护下
        /// 继续运行引擎（Windows 没有进程组兜底）。
        pub fn create_for_process(pid: u32) -> io::Result<Self> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
                if handle.is_null() {
                    return Err(win32_error("CreateJobObjectW"));
                }

                let mut info = JobObjectExtendedLimitInformation::default();
                info.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let set_ok = SetInformationJobObject(
                    handle,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                    &info as *const JobObjectExtendedLimitInformation as *const c_void,
                    std::mem::size_of::<JobObjectExtendedLimitInformation>() as u32,
                );
                if set_ok == FALSE {
                    let err = win32_error("SetInformationJobObject");
                    CloseHandle(handle);
                    return Err(err);
                }

                let process = OpenProcess(
                    PROCESS_SET_QUOTA | PROCESS_SET_INFORMATION | PROCESS_TERMINATE,
                    FALSE,
                    pid,
                );
                if process.is_null() {
                    let err = win32_error("OpenProcess");
                    CloseHandle(handle);
                    return Err(err);
                }
                let assign_ok = AssignProcessToJobObject(handle, process);
                if assign_ok == FALSE {
                    let err = win32_error("AssignProcessToJobObject");
                    CloseHandle(process);
                    CloseHandle(handle);
                    return Err(err);
                }
                CloseHandle(process);
                Ok(Self { handle })
            }
        }

        /// Windows 上引擎一定持有 Job 句柄。
        pub fn attached(&self) -> bool {
            true
        }
    }

    impl Drop for JobObject {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                // SAFETY: 句柄由本对象独占；关闭即触发 kill-on-close，回收整棵树。
                unsafe {
                    CloseHandle(self.handle);
                }
                self.handle = std::ptr::null_mut();
            }
        }
    }
}

pub use imp::JobObject;

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn create_for_process_is_noop_without_windows() {
        let job = JobObject::create_for_process(4242).expect("非 Windows 应直接成功");
        assert!(!job.attached(), "非 Windows 不持有 Job 句柄");
    }

    #[test]
    fn job_object_is_zero_sized_without_windows() {
        assert_eq!(std::mem::size_of::<JobObject>(), 0, "非 Windows 实现零开销");
    }

    #[test]
    fn dropping_job_object_without_windows_is_safe() {
        drop(JobObject::create_for_process(1).unwrap());
        drop(JobObject::default());
    }
}
