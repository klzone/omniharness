/**
 * OmniDesk 品牌标（OD-ARCH-081）。
 *
 * 复用 `public/omnidesk-monomark.svg` 的 O+D 环形结构（O = Runtime 循环，D = 工作台面，
 * 中间竖线是调度缝，五个节点对应五类能力）。OD-UI-717 起改为**单色描边**：
 * 去掉渐变与发光，只跟随所在文字颜色，小尺寸下更清楚，也不再与纸面材质冲突。
 *
 * 纯装饰：不接收点击、不带语义，调用方不要用它当按钮。
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
      {/* O：Runtime 循环 */}
      <rect
        x="104"
        y="128"
        width="156"
        height="256"
        rx="78"
        fill="none"
        stroke="currentColor"
        strokeWidth="34"
      />
      {/* D：工作台面 */}
      <path
        d="M 236 128 H 280 C 350.69 128 408 185.31 408 256 C 408 326.69 350.69 384 280 384 H 236"
        fill="none"
        stroke="currentColor"
        strokeWidth="34"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      {/* 中央调度缝 */}
      <line x1="248" y1="176" x2="248" y2="336" stroke="currentColor" strokeWidth="16" strokeLinecap="round" />
      {/* 五个能力节点 */}
      <circle cx="182" cy="128" r="13" fill="currentColor" />
      <circle cx="182" cy="384" r="13" fill="currentColor" />
      <circle cx="248" cy="256" r="17" fill="currentColor" />
      <circle cx="344" cy="154" r="13" fill="currentColor" />
      <circle cx="344" cy="358" r="13" fill="currentColor" />
    </svg>
  );
}
