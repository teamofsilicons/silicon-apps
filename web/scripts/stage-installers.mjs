import { copyFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

// Keep hosted bootstrap scripts byte-identical to the reviewed release sources.
for (const name of ["install.sh", "install.ps1"]) {
  await copyFile(
    fileURLToPath(new URL(`../../scripts/${name}`, import.meta.url)),
    fileURLToPath(new URL(`../dist/${name}`, import.meta.url)),
  );
}
