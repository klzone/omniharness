import { createElectronBuilderConfig } from '/home/kevin/dsh-build/deepseek-harness-dsh-v0.2.1-alpha.2/apps/desktop/scripts/electron-builder-config.mjs'
const base = createElectronBuilderConfig()
export default {
  ...base,
  linux: {
    ...(base.linux ?? {}),
    target: ['deb'],
    maintainer: 'DeepSeek <dev@deepseek.com>',
    syncDesktopName: true,
  },
  deb: {
    ...(base.deb ?? {}),
    packageName: 'deepseek-harness',
    afterInstall: 'scripts/linux-deb-after-install.sh',
  },
}
