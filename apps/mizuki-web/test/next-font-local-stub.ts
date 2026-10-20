// next/font/local also runs only in the Next build pipeline. Reuse the existing
// loader shape for metadata tests; production builds use the real font loader.
export { Archivo as default } from './next-font-google-stub';
