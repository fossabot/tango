import * as crc32 from "crc-32";
import { readdir, readFile } from "fs/promises";
import mkdirp from "mkdirp";
import path from "path";

export const KNOWN_ROM_FAMILIES = require("./roms.json5").default as {
  [family: string]: {
    title: { [language: string]: string };
    versions: { [name: string]: KnownROM };
  };
};

export const FAMILY_BY_ROM_NAME = (() => {
  const FAMILY_BY_ROM_NAME: { [romName: string]: string } = {};
  for (const family of Object.keys(KNOWN_ROM_FAMILIES)) {
    for (const version of Object.keys(KNOWN_ROM_FAMILIES[family].versions)) {
      FAMILY_BY_ROM_NAME[version] = family;
    }
  }
  return FAMILY_BY_ROM_NAME;
})();

export interface ROMInfo {
  name: string;
  crc32: number;
}

const decoder = new TextDecoder("ascii");

export function getROMInfo(buffer: ArrayBuffer) {
  const name = decoder.decode(new Uint8Array(buffer, 0x000000a0, 16));
  return { name, crc32: crc32.buf(new Uint8Array(buffer)) >>> 0 };
}

export interface KnownROM {
  title: { [language: string]: string };
  revisions: { [key: string]: { crc32: number } };
  netplayCompatibility: string;
}

export async function scan(dir: string) {
  const games = {} as {
    [name: string]: string;
  };

  let filenames: string[];
  try {
    filenames = await readdir(dir);
  } catch (e) {
    if ((e as any).code == "ENOENT") {
      await mkdirp(dir);
      filenames = [];
    } else {
      throw e;
    }
  }

  for (const result of await Promise.allSettled(
    filenames.map(async (filename) => {
      try {
        const romInfo = getROMInfo(
          (await readFile(path.join(dir, filename))).buffer
        );
        if (
          !Object.prototype.hasOwnProperty.call(
            FAMILY_BY_ROM_NAME,
            romInfo.name
          )
        ) {
          throw `unknown rom name: ${romInfo.name}`;
        }

        const familyName = FAMILY_BY_ROM_NAME[romInfo.name];
        const family = KNOWN_ROM_FAMILIES[familyName];
        const rom = family.versions[romInfo.name];

        const crc32s = Object.values(rom.revisions).map(
          (revision) => revision.crc32
        );

        if (crc32s.indexOf(romInfo.crc32) == -1) {
          throw `mismatched crc32: expected one of ${crc32s
            .map((crc32) => crc32.toString(16).padStart(8, "0"))
            .join(", ")}, got ${romInfo.crc32.toString(16).padStart(8, "0")}`;
        }

        games[romInfo.name] = filename;
      } catch (e) {
        throw `failed to scan rom ${filename}: ${e}`;
      }
    })
  )) {
    if (result.status == "rejected") {
      console.warn("rom skipped:", result.reason);
    }
  }
  return games;
}
