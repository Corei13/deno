import { ReframeNS } from "ext:core/ops";

export default {
  analyze(path, content, env = "server") {
    const json = ReframeNS.analyze(path, content, env);
    return JSON.parse(json);
  },
};
