import React from "react";
import { useTranslation } from "react-i18next";

import Box from "@mui/material/Box";
import Chip from "@mui/material/Chip";
import Stack from "@mui/material/Stack";
import Table from "@mui/material/Table";
import TableBody from "@mui/material/TableBody";
import TableCell from "@mui/material/TableCell";
import TableRow from "@mui/material/TableRow";

import { ModcardsEditor } from "../../saveedit";
import { fallbackLng } from "../i18n";

const DEBUFF_COLOR = "#b55ade";
const BUFF_COLOR = "#ffbd18";
const OFF_COLOR = "#bdbdbd";

function gameVersion(romName: string) {
  switch (romName) {
    case "ROCKEXE6_RXXBR6J":
      return "falzar";
    case "ROCKEXE6_GXXBR5J":
      return "gregar";
  }
  throw `unknown rom name: ${romName}`;
}

export default function ModcardsViewer({
  editor,
  romName,
  active,
}: {
  editor: ModcardsEditor;
  romName: string;
  active: boolean;
}) {
  const { i18n } = useTranslation();

  const modcards: { id: number; enabled: boolean }[] = [];
  for (let i = 0; i < editor.getModcardCount(); i++) {
    modcards.push(editor.getModcard(i)!);
  }

  return (
    <Box
      flexGrow={1}
      display={active ? "block" : "none"}
      overflow="auto"
      sx={{ px: 1, height: 0 }}
    >
      <Table size="small">
        <TableBody>
          {modcards.map(({ id, enabled }, i) => {
            const modcard = editor.getModcardData()[id];
            if (modcard == null) {
              return null;
            }

            return (
              <TableRow key={i}>
                <TableCell>
                  {modcard.name[
                    i18n.resolvedLanguage as keyof typeof modcard.name
                  ] ||
                    modcard.name[fallbackLng as keyof typeof modcard.name]}{" "}
                  <small>{modcard.mb}MB</small>
                </TableCell>
                <TableCell sx={{ verticalAlign: "top", width: "25%" }}>
                  <Stack spacing={0.5}>
                    {modcard.parameters.flatMap((l, i) =>
                      l.version == null || l.version == gameVersion(romName)
                        ? [
                            <Chip
                              key={i}
                              label={
                                l.name[
                                  i18n.resolvedLanguage as keyof typeof l.name
                                ] || l.name[fallbackLng as keyof typeof l.name]
                              }
                              size="small"
                              sx={{
                                fontSize: "0.9rem",
                                justifyContent: "flex-start",
                                color: "black",
                                backgroundColor: enabled
                                  ? l.debuff
                                    ? DEBUFF_COLOR
                                    : BUFF_COLOR
                                  : OFF_COLOR,
                              }}
                            />,
                          ]
                        : []
                    )}
                  </Stack>
                </TableCell>
                <TableCell sx={{ verticalAlign: "top", width: "25%" }}>
                  <Stack spacing={0.5}>
                    {modcard.abilities.flatMap((l, i) =>
                      l.version == null || l.version == gameVersion(romName)
                        ? [
                            <Chip
                              key={i}
                              label={
                                l.name[
                                  i18n.resolvedLanguage as keyof typeof l.name
                                ] || l.name[fallbackLng as keyof typeof l.name]
                              }
                              size="small"
                              sx={{
                                fontSize: "0.9rem",
                                justifyContent: "flex-start",
                                color: "black",
                                backgroundColor: enabled
                                  ? l.debuff
                                    ? DEBUFF_COLOR
                                    : BUFF_COLOR
                                  : OFF_COLOR,
                              }}
                            />,
                          ]
                        : []
                    )}
                  </Stack>
                </TableCell>
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
    </Box>
  );
}
