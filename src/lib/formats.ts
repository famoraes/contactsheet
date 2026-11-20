// Mirrors crates/negative/src/raw.rs RAW_EXTENSIONS.
export const RAW_EXTENSIONS = [
  "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "rwl", "pef", "srw", "3fr",
  "fff", "iiq", "erf", "mef", "mos", "mrw", "kdc", "dcr",
];
export const TIFF_EXTENSIONS = ["tif", "tiff"];
export const SCAN_EXTENSIONS = [...TIFF_EXTENSIONS, ...RAW_EXTENSIONS];

/** True for files ContactSheet can open (TIFF scans, camera RAW, DNG). */
export const isScanFile = (path: string) => {
  const m = /\.([^./\\]+)$/.exec(path);
  return !!m && SCAN_EXTENSIONS.includes(m[1].toLowerCase());
};
