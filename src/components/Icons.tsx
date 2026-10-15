// Inline stroke icons (24px grid, currentColor). Drawn for this launcher.
import type { SVGProps } from "react";

type P = SVGProps<SVGSVGElement> & { size?: number };

function Svg({ size = 20, children, ...rest }: P) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.9}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...rest}
    >
      {children}
    </svg>
  );
}

export const IconGear = (p: P) => (
  <Svg {...p}>
    <path d="M10.3 2.6h3.4l.6 2.6a7.6 7.6 0 0 1 1.9 1.1l2.5-.8 1.7 2.9-2 1.8a7.7 7.7 0 0 1 0 2.2l2 1.8-1.7 2.9-2.5-.8a7.6 7.6 0 0 1-1.9 1.1l-.6 2.6h-3.4l-.6-2.6a7.6 7.6 0 0 1-1.9-1.1l-2.5.8-1.7-2.9 2-1.8a7.7 7.7 0 0 1 0-2.2l-2-1.8 1.7-2.9 2.5.8a7.6 7.6 0 0 1 1.9-1.1z" />
    <circle cx="12" cy="12" r="3" />
  </Svg>
);

export const IconHelp = (p: P) => (
  <Svg {...p}>
    <path d="M12 2.8 20 7.4v9.2l-8 4.6-8-4.6V7.4z" />
    <path d="M9.6 9.6a2.5 2.5 0 1 1 3.4 2.3c-.6.3-1 .8-1 1.5v.4" />
    <circle cx="12" cy="16.6" r=".4" fill="currentColor" />
  </Svg>
);

export const IconPlay = (p: P) => (
  <Svg {...p}>
    <path d="M7 4.5v15l12.5-7.5z" fill="currentColor" stroke="none" />
  </Svg>
);

export const IconDisc = (p: P) => (
  <Svg {...p}>
    <circle cx="12" cy="12" r="9" />
    <circle cx="12" cy="12" r="2.4" />
    <path d="M16.8 7.2a6.6 6.6 0 0 1 1.8 3.4" />
  </Svg>
);

export const IconFolder = (p: P) => (
  <Svg {...p}>
    <path d="M3 6.5V18a1.5 1.5 0 0 0 1.5 1.5h15A1.5 1.5 0 0 0 21 18V9a1.5 1.5 0 0 0-1.5-1.5h-7.2L10.2 5H4.5A1.5 1.5 0 0 0 3 6.5z" />
  </Svg>
);

export const IconShield = (p: P) => (
  <Svg {...p}>
    <path d="M12 3 5 5.8v5.4c0 4.4 3 8.1 7 9.8 4-1.7 7-5.4 7-9.8V5.8z" />
    <path d="m8.8 12 2.2 2.2 4.2-4.4" />
  </Svg>
);

export const IconRefresh = (p: P) => (
  <Svg {...p}>
    <path d="M20 11a8 8 0 0 0-14.6-4.4M4 4v4h4" />
    <path d="M4 13a8 8 0 0 0 14.6 4.4M20 20v-4h-4" />
  </Svg>
);

export const IconTrash = (p: P) => (
  <Svg {...p}>
    <path d="M4 7h16M9.5 7V4.5h5V7M6.5 7l.9 12.5h9.2L17.5 7M10 11v5M14 11v5" />
  </Svg>
);

export const IconDots = (p: P) => (
  <Svg {...p}>
    <circle cx="5.5" cy="12" r="1.3" fill="currentColor" />
    <circle cx="12" cy="12" r="1.3" fill="currentColor" />
    <circle cx="18.5" cy="12" r="1.3" fill="currentColor" />
  </Svg>
);

export const IconChip = (p: P) => (
  <Svg {...p}>
    <rect x="6" y="6" width="12" height="12" rx="1.5" />
    <path d="M9.5 9.5h5v5h-5zM9.5 3v3M14.5 3v3M9.5 18v3M14.5 18v3M3 9.5h3M3 14.5h3M18 9.5h3M18 14.5h3" />
  </Svg>
);

export const IconDownload = (p: P) => (
  <Svg {...p}>
    <path d="M12 4v11M7 10.5l5 5 5-5M5 19.5h14" />
  </Svg>
);

export const IconPlus = (p: P) => (
  <Svg {...p}>
    <path d="M12 5v14M5 12h14" />
  </Svg>
);

export const IconCheck = (p: P) => (
  <Svg {...p}>
    <path d="m5 12.5 4.5 4.5L19 7.5" />
  </Svg>
);

export const IconWarn = (p: P) => (
  <Svg {...p}>
    <path d="M12 3.5 21.5 20h-19z" />
    <path d="M12 10v4.5" />
    <circle cx="12" cy="17.2" r=".5" fill="currentColor" />
  </Svg>
);

export const IconInfo = (p: P) => (
  <Svg {...p}>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5.5" />
    <circle cx="12" cy="7.8" r=".5" fill="currentColor" />
  </Svg>
);

export const IconClose = (p: P) => (
  <Svg {...p}>
    <path d="m6 6 12 12M18 6 6 18" />
  </Svg>
);

export const IconMinus = (p: P) => (
  <Svg {...p}>
    <path d="M5 12h14" />
  </Svg>
);

export const IconSquare = (p: P) => (
  <Svg {...p}>
    <rect x="5.5" y="5.5" width="13" height="13" rx="1" />
  </Svg>
);

export const IconLink = (p: P) => (
  <Svg {...p}>
    <path d="M14 4h6v6M20 4l-9 9M18 14v4.5A1.5 1.5 0 0 1 16.5 20h-11A1.5 1.5 0 0 1 4 18.5v-11A1.5 1.5 0 0 1 5.5 6H10" />
  </Svg>
);

export const IconFile = (p: P) => (
  <Svg {...p}>
    <path d="M6 3h8l4 4v14H6z" />
    <path d="M14 3v4h4" />
  </Svg>
);

/** Small hex bolt head used as a decorative rivet. */
export function Rivet({ className }: { className?: string }) {
  return (
    <svg className={className} width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
      <path d="M5 .6 8.8 2.8v4.4L5 9.4 1.2 7.2V2.8z" fill="#5d6980" stroke="#1a2130" strokeWidth=".8" />
      <path d="M3 5h4" stroke="#1a2130" strokeWidth="1" />
    </svg>
  );
}
