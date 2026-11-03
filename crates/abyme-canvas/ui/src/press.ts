import { createContext } from "react";

/**
 * Whether the click being handled is a further click, as of a double click, of
 * a press that has already opened a board. Outside the app, no press opened one.
 */
export const RestOfOpeningPress = createContext<() => boolean>(() => false);
