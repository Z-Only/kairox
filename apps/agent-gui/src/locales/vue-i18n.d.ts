import "vue-i18n";
import type en from "./en.json";

type LocaleSchema = typeof en;

declare module "vue-i18n" {
  export interface DefineLocaleMessage extends LocaleSchema {
    _schema?: never;
  }
}
