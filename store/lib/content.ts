/**
 * The home page's questions, in the Carbon's words (store/llms/llms.md), shared by the FAQ section and its FAQPage
 * JSON-LD. Answers are plain text; `code` in backticks and https:// links are formatted where they are shown.
 */
import { LINKS } from "./site";

export interface Faq {
  id: string;
  question: string;
  answer: string;
}

export const FAQ: Faq[] = [
  {
    id: "what-is-silicon-apps",
    question: "What is Silicon Apps?",
    answer: "Silicon Apps is the store of the Silicon ecosystem. Every app here is made for both Silicons and Carbons to use, and every app is a CLI first, so a Silicon can install it, run it and find its way around it on its own. Many apps also have a website and mobile apps linked to them.",
  },
  {
    id: "install-an-app",
    question: "How do I install an app?",
    answer: "Install the `silicon-apps` CLI once, then run `silicon-apps install` with the app's id, for example `silicon-apps install briefcase`. It picks the right package for your OS and architecture, checks its checksum, installs the command and tells you how to run it.",
  },
  {
    id: "do-i-need-an-account",
    question: "Do I need an account?",
    answer: "Not to find or install a public app. You sign in with your Silicon Accounts account for private apps shared with you and for reviews. As a Silicon, run `silicon-accounts login --app silicon-apps`, then `silicon-apps login --slt TOKEN`.",
  },
  {
    id: "updates",
    question: "How do apps stay up to date?",
    answer: "We check for a new release of every installed app every minute and update it on the channel you installed it from, production or development. Apps never run an updater of their own, and `silicon-apps` updates itself the same way.",
  },
  {
    id: "versions",
    question: "Can I install a development release or an exact version?",
    answer: "Yes. Development releases install as `{app_id}>dev`, for example `silicon-apps install 'briefcase>dev'`, and an exact version with `@`, for example `silicon-apps install 'briefcase@3.4.2'`. Updates then follow that channel.",
  },
  {
    id: "three-commands",
    question: "How does a Silicon find its way around a new app?",
    answer: "Every app answers the same three commands on every system: `--help` for what it does and every command it has, `accounts --json` for its app id and details, and `login status --json` for whether you are signed in and as whom. Start every new app with its `--help`.",
  },
  {
    id: "from-code",
    question: "Can a Silicon use the store without a browser?",
    answer: "Yes. Everything here is plain HTML and an API. Read https://apps.teamofsilicons.com/llms.txt, or call the Apps API at https://apps.teamofsilicons.com/v1.",
  },
  {
    id: "open-source",
    question: "Is Silicon Apps open source?",
    answer: `Yes. Silicon Apps and Silicon Accounts are both open source under the MIT license, so we have nothing to hide. Read the code at ${LINKS.appsGithub} and ${LINKS.accountsGithub}. If something is wrong, tell us with \`silicon-apps report\`, and add \`--pr\` with a link if you have already fixed it.`,
  },
];

export const plainAnswer = (faq: Faq) => faq.answer.replace(/`/g, "");
