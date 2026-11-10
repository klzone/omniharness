// OD-PROC-149b：通过 `node --require <本文件>` 注入，把 DSH 进程标题改成
// omnidesk-dsh，便于在 macOS 活动监视器 / ps 里认出归属。
// 只改 Node 进程的 argv[0] 展示名，不改 DSH 源码与运行行为；
// macOS/Linux 上 ps、活动监视器可见，Windows 的任务管理器忽略该标题。
process.title = "omnidesk-dsh";
