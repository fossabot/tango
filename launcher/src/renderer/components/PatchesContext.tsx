import React, { useContext } from "react";

import { app } from "@electron/remote";

import { PatchInfos, scan, update } from "../../patch";
import { getPatchesPath } from "../../paths";
import { useConfig } from "./ConfigContext";

export interface PatchesValue {
  rescan(): Promise<void>;
  update(): Promise<void>;
  updating: boolean;
  patches: PatchInfos;
}

const Context = React.createContext(null! as PatchesValue);

function makeScanPatches() {
  let status: "pending" | "error" | "ok" = "pending";
  let result: PatchesValue["patches"];
  let err: any;
  const promise = (async () => {
    try {
      result = await scan(getPatchesPath(app));
    } catch (e) {
      console.error(e);
      err = e;
      status = "error";
    }
    status = "ok";
  })();
  return () => {
    switch (status) {
      case "pending":
        throw promise;
      case "error":
        throw err;
      case "ok":
        return result;
    }
  };
}

const scanPatches = makeScanPatches();

export const PatchesProvider = ({
  children,
}: {
  children?: React.ReactNode;
} = {}) => {
  const { config } = useConfig();
  const [currentPatches, setCurrentPatches] = React.useState(scanPatches());
  const [updating, setUpdating] = React.useState(false);
  const rescan = async () => {
    try {
      setCurrentPatches(await scan(getPatchesPath(app)));
    } catch (e) {
      console.error(e);
    }
  };

  return (
    <Context.Provider
      value={{
        rescan,
        async update() {
          try {
            setUpdating(true);
            await update(getPatchesPath(app), config.patchRepo);
            await rescan();
          } catch (e) {
            console.error("failed to update patches", e);
          } finally {
            setUpdating(false);
          }
        },
        patches: currentPatches,
        updating,
      }}
    >
      {children}
    </Context.Provider>
  );
};

export const PatchesConsumer = Context.Consumer;

export function usePatches() {
  return useContext(Context);
}
