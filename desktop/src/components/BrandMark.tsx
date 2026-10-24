/**
 * OmniDesk 品牌标（OD-ARCH-081）。
 *
 * 复用 `public/omnidesk-monomark.svg` 的 O+D 环形结构（O = Runtime 循环，D = 工作台面，
 * 中间竖线是调度缝，五个节点对应五类能力），但**去掉深色圆角底**：在浅色输入区里
 * 带一块深色方底会显得突兀，透明底的小尺寸标识更干净。
 *
 * 纯装饰：不接收点击、不带语义，调用方不要用它当按钮。
 * 渐变 id 加了前缀，避免同页面多次渲染时 SVG id 冲突。
 */
export function BrandMark({ className = "" }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 512 512"
      className={className}
      role="img"
      aria-label="OmniDesk"
      focusable="false"
    >
      <defs>
        <linearGradient id="odMarkOmni" x1="120" y1="120" x2="300" y2="392" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="#38BDF8" />
          <stop offset="100%" stopColor="#818CF8" />
        </linearGradient>
        <linearGradient id="odMarkDesk" x1="220" y1="120" x2="400" y2="392" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="#818CF8" />
          <stop offset="100%" stopColor="#C084FC" />
        </linearGradient>
        <linearGradient id="odMarkBeam" x1="248" y1="168" x2="248" y2="344" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="#38BDF8" />
          <stop offset="100%" stopColor="#C084FC" />
        </linearGradient>
      </defs>
      {/* O：Runtime 循环 */}
      <rect
        x="104"
        y="128"
        width="156"
        height="256"
        rx="78"
        fill="none"
        stroke="url(#odMarkOmni)"
        strokeWidth="34"
      />
      {/* D：工作台面 */}
      <path
        d="M 236 128 H 280 C 350.69 128 408 185.31 408 256 C 408 326.69 350.69 384 280 384 H 236"
        fill="none"
        stroke="url(#odMarkDesk)"
        strokeWidth="34"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      {/* 中央调度缝 */}
      <line x1="248" y1="176" x2="248" y2="336" stroke="url(#odMarkBeam)" strokeWidth="16" strokeLinecap="round" />
      {/* 五个能力节点 */}
      <circle cx="182" cy="128" r="13" fill="#38BDF8" />
      <circle cx="182" cy="384" r="13" fill="#818CF8" />
      <circle cx="248" cy="256" r="17" fill="#4F46E5" />
      <circle cx="344" cy="154" r="13" fill="#818CF8" />
      <circle cx="344" cy="358" r="13" fill="#C084FC" />
    </svg>
  );
}
