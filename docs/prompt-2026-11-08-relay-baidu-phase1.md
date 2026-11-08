# 接力 Prompt（百度网盘接入 AFM · 阶段 1 代码侧已收口，下一步 = 真实会话补验 + 阶段 2 上传）

把下面整段原样发给新会话即可，**不要改写、不要复述 handoff 内容**：

```
接手 Alpha File Manager（AFM）百度网盘接入阶段 1 的后续开发；开工前按顺序完整读这五份文档自己掌握背景、目标、进度与下阶段计划，不要等我复述：1) F:\soft\00selfmade\filemanager\AGENTS.md 2) F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-08-baidu-netdisk-phase1.md 3) F:\soft\00selfmade\filemanager\sigma-file-manager\docs\2026-11-08-baidu-t9-acceptance.md 4) F:\soft\00selfmade\filemanager\sigma-file-manager\docs\plan-2026-11-08-baidu-netdisk-phase1.md 5) F:\soft\00selfmade\filemanager\sigma-file-manager\docs\2026-11-07-baidu-t1-auth-findings.md；代码仓在 F:\soft\00selfmade\filemanager\sigma-file-manager，分支 feat/baidu-remote-provider。
```

说明（不随 prompt 发出）：

1. 第 1 份是仓库硬约束；第 2 份是交接文档（阶段 1 全量上下文：T1 结论、T4–T9 交付说明、已验证/推断/未验证三分、高危铁律、工具坑、证据路径、§12 下一会话三条线、§13 本轮交付与踩坑）；第 3 份是 T9 验收清单与逐条实测结果；第 4 份是阶段 1 实现计划；第 5 份是 T1 认证实测记录（U1/U2/U3 三问结论）。
2. 计划文档里的 **D2（换 `refresh_token`）已被 T1 实测否决**，认证路线是「cookie 直连」；以 handoff §11 与 T1 记录为准，不要照 D2 实现。
3. **开工第一件事**是按 handoff §10 回滚 43 个与本特性无关的 `cargo fmt` 格式化文件，否则提交会带进一堆无关改动；**不要**再跑 `cargo fmt`。
4. 阶段 1 的打包与 UI 取证已完成（`BUILD_EXITCODE=0`），**剩下的是依赖真实百度会话的 5 条判定**（列表/新建/改名/删除/下载）——本机没有百度会话，**不得写成「通过」**，须在有会话的机器上补验。
