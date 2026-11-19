"use client";
/**
 * The search field: Silicon UI's SearchField (its glass, its clear button), holding what is typed so the form can send
 * it as `q`. Server-rendered with the current search, so without script it is a plain search field in a GET form.
 */
import { useState } from "react";
import { SearchField } from "@teamofsilicons/silicon-ui/search-field";

export function SearchInput({ id, defaultValue = "", label = "Search apps" }: { id: string; defaultValue?: string; label?: string }) {
  const [value, setValue] = useState(defaultValue);
  return (
    <SearchField
      id={id}
      name="q"
      label={label}
      value={value}
      onValueChange={setValue}
      placeholder="Search apps"
      autoComplete="off"
      spellCheck={false}
      enterKeyHint="search"
      maxLength={200}
      aria-keyshortcuts="/"
    />
  );
}
