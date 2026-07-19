# SpriteAnime CLI

`sprite-anime-cli` 是桌面应用的纯批处理入口，直接复用 Rust 配置、API、图像处理、FFmpeg、精灵拆分和分组重绘实现。它不启动 WebView，也不会修改 `workspace.json`。

## 启动与数据目录

开发环境运行：

```bash
npm run cli -- --help
npm run cli -- --data-dir ./SpriteAnimteData config validate
```

数据目录优先级为 `--data-dir`、`SPRITE_ANIME_DATA_DIR`、可执行文件旁的 `SpriteAnimteData/`。CLI 与桌面应用指向同一目录时，共享配置、素材库、工作台、日志、工具和重绘清单。修改操作使用非阻塞跨进程锁；数据正被占用时立即以退出码 `4` 失败。

全局参数：

```text
--data-dir PATH
--profile ID
--format human|json
--json
--quiet
--no-record
```

JSON 成功或失败结果使用 `schemaVersion: 1`；长任务进度以 JSON Lines 写入 stderr。API 密钥只能来自所选配置组或以下环境变量，不接受命令行密钥参数：

```text
SPRITE_ANIME_IMAGE_API_KEY
SPRITE_ANIME_VIDEO_API_KEY
SPRITE_ANIME_OPTIMIZER_API_KEY
```

## 配置与本地数据

```bash
sprite-anime-cli config show --json
sprite-anime-cli config profile list
sprite-anime-cli config profile add local "本地服务" \
  --image-base http://127.0.0.1:8000/v1 \
  --image-mode images_generations --image-model grok-imagine-image
SPRITE_ANIME_IMAGE_API_KEY=... \
  sprite-anime-cli config secret set image --profile local
sprite-anime-cli config profile activate local
sprite-anime-cli config export ./config-redacted.json
sprite-anime-cli config export ./config-full.json --include-secrets --yes
sprite-anime-cli config import ./config-full.json --yes
```

默认导出保持 `config.json` 的可导入 schema，但清空三类密钥。完整导出必须同时指定 `--include-secrets --yes`，Unix 文件权限设为 `0600`。

其他本地命令：

```bash
sprite-anime-cli presets list
sprite-anime-cli history list --limit 20
sprite-anime-cli workbench list --limit 50
sprite-anime-cli assets import-image ./reference.png
sprite-anime-cli assets list --category generated-images
sprite-anime-cli tools check
sprite-anime-cli tools install --proxy http://127.0.0.1:7890
sprite-anime-cli workspace validate
sprite-anime-cli logs tail --file video-sprite.log --lines 100
```

清空、删除、覆盖配置、丢弃重绘等不可撤销操作必须添加 `--yes`。

## API、图片与视频

```bash
sprite-anime-cli api check image
sprite-anime-cli api check video
sprite-anime-cli api check optimizer

sprite-anime-cli prompt optimize --prompt "角色奔跑" --rows 4 --cols 4
sprite-anime-cli image generate --prompt "角色奔跑序列帧" \
  --ratio 1:1 --resolution 1K --count 1 --output ./images
sprite-anime-cli image matte ./images/input.png --mode auto --output ./matted
sprite-anime-cli image erase ./images/input.png \
  --operations ./erase-operations.json --output ./matted

sprite-anime-cli video generate --prompt "角色原地奔跑" \
  --size 1280x720 --seconds 4 --output ./videos
sprite-anime-cli video probe ./videos/input.mp4
sprite-anime-cli video extract ./videos/input.mp4 --frames 16 \
  --start 0 --end 4 --output ./frames
sprite-anime-cli video preview ./videos/input.mp4 --frames 16 --cols 4 \
  --output ./preview.png
```

生成约束可从严格 JSON 文件加载：图片使用 `--constraints image-constraints.json --grid-rows 4 --grid-cols 4`，视频和重绘使用 `--constraints ...`。字段与工作区中的 `generationConstraints`/`constraints` 相同。

擦除操作采用唯一的 `EraseOperationsV1`：

```json
{
  "schemaVersion": 1,
  "operations": [
    { "x": 120, "y": 80, "tolerance": 28, "radius": 1 }
  ]
}
```

## 精灵图与分组重绘

```bash
sprite-anime-cli sprite detect ./sheet.png --rows 4 --cols 4 \
  --allow-expand --output ./layout.json
sprite-anime-cli sprite split ./sheet.png --layout ./layout.json \
  --mode fixed --output ./frames
sprite-anime-cli sprite preview ./sheet.png --layout ./layout.json \
  --mode tight --output ./sprite-preview.png
sprite-anime-cli sprite export-frames ./frames --prefix run --output ./exported
sprite-anime-cli sprite export-gif ./frames --name run --fps 12 --output ./gifs

sprite-anime-cli redraw start --frames-dir ./frames --final-cols 4 \
  --group-rows 2 --group-cols 2 --prompt "保持角色一致并精修" --end 1
sprite-anime-cli redraw run
sprite-anime-cli redraw status --json
sprite-anime-cli redraw resume
sprite-anime-cli redraw set-final-cols 8
sprite-anime-cli redraw finalize --output ./final.png
sprite-anime-cli redraw discard --yes
```

`redraw run` 严格按批次顺序执行并逐批保存状态；可恢复失败返回退出码 `8`。第一批只上传当前网格，后续批次按顺序上传“当前网格 + 上一批生成末帧”，以保持角色和动作连续性，不增加批次数或输出帧。最后一批不足一组时自动复制末帧作为占位，只拆回真实帧数。分组重绘不支持纯文本 `/images/generations`；请使用 Responses、Chat Completions 或 `/images/edits` JSON/multipart。上游拒绝多参考图时当前批次明确失败并暂停，不会自动拼图或降级。`finalize` 仅重新排版已成功帧，不再次调用 API。

## 退出码

| 代码 | 含义 |
| ---: | --- |
| 0 | 成功 |
| 2 | 参数或输入校验失败 |
| 3 | 配置、schema 或配置组错误 |
| 4 | 跨进程数据锁占用 |
| 5 | 文件系统或工具错误 |
| 6 | API 或上游错误 |
| 7 | 媒体处理失败 |
| 8 | 已保存状态的可恢复部分失败 |
| 9 | 内部不变量失败 |
