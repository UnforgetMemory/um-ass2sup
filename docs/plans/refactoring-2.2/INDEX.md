# 重构 2.2 — subtitle-renderer 重建 + PGS 编码器补完

> 接续 2.1（ass-core 完成）。消费 `SubtitleDocument`，重建渲染引擎，覆盖 53 OverrideTag 特效，事件级崩溃隔离，帧级别精确。

## 全链路定位

```
2.1 (ass-core)        2.2 (当前层)                2.3 (编码层)          2.4 (管线)
ASS/SRT → AST         AST → RGBA位图             位图 → PGS段          管线整合
SubtitleDocument      RenderedFrame              Vec<Segment>          .sup 文件
```

## 文档导航

| 文档 | 内容 |
|------|------|
| `PLAN_SPEC.md` | 完整规格：19 任务、4 波次、模块设计、字体策略、日志方案、退出标准 |
| `TASKS.md` | 原子任务列表 + 执行顺序 |
