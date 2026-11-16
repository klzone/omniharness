# 第三方软件声明

OmniDesk 自有代码的许可证见 [LICENSE](LICENSE)。`runtime/` 目录中的第三方组件、上游补丁及依赖不在 PolyForm 非商业许可范围内，按各自许可证分发。

## DeepSeek Harness

- 项目：DeepSeek Harness
- 上游：https://github.com/deepseek-ai/deepseek-harness
- 锁定版本：`dsh-v0.2.1-rc.5`
- 锁定提交：`b830ec950faba0a46dda564be3e2d854afed94f1`
- 许可证：MIT
- 版权：Copyright (c) 2026 DeepSeek
- 本地路径：`runtime/deepseek-harness`（Git submodule）

分发 OmniDesk 安装包或其他包含 DeepSeek Harness 的产物时，必须一并保留本声明、上游 MIT 许可证和版权声明。

### MIT 许可证文本

```
MIT License

Copyright (c) 2026 DeepSeek

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## 其他依赖

`runtime/`、安装包和开发依赖中还包含其他 npm、Python 第三方包。各包的许可证文本保留在对应包目录内；打包分发时应一并包含其许可证与 NOTICE 要求。
