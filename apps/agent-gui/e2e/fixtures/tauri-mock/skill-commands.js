/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- skill commands ---- */

registerCommandHandlers({
  list_skills: function (args) {
    return Promise.resolve(state.skills);
  },
  get_skill_detail: function (args) {
    var detailSkill = state.skills.find(function (skill) {
      return skill.id === args.skillId;
    });
    if (!detailSkill) return Promise.reject(new Error("Skill not found: " + args.skillId));
    return Promise.resolve({
      view: detailSkill,
      body_markdown: "# " + detailSkill.name + "\n\n" + detailSkill.description
    });
  },
  activate_skill: function (args) {
    var skillToActivate = state.skills.find(function (skill) {
      return skill.id === args.skillId;
    });
    if (!skillToActivate) return Promise.reject(new Error("Skill not found: " + args.skillId));
    var activeSkill = {
      skill_id: skillToActivate.id,
      name: skillToActivate.name,
      source: skillToActivate.source,
      activation_mode: skillToActivate.activation_mode
    };
    state.activeSkills = state.activeSkills.filter(function (skill) {
      return skill.skill_id !== activeSkill.skill_id;
    });
    state.activeSkills.push(activeSkill);
    return Promise.resolve(activeSkill);
  },
  deactivate_skill: function (args) {
    state.activeSkills = state.activeSkills.filter(function (skill) {
      return skill.skill_id !== args.skillId;
    });
    return Promise.resolve(null);
  },
  list_active_skills: function (args) {
    return Promise.resolve(state.activeSkills);
  },
  list_skill_settings: function (args) {
    return clone(state.skillSettings);
  },
  get_effective_skills: function (args) {
    return clone(state.skillSettings.map(effectiveSkillView));
  },
  get_skill_settings_detail: function (args) {
    var detailSetting = findSkillSetting(args.skillId);
    if (!detailSetting)
      return Promise.reject(new Error("Skill setting not found: " + args.skillId));
    return {
      view: clone(detailSetting),
      content: "# " + detailSetting.name + "\n\n" + detailSetting.description,
      source_chain: [clone(detailSetting)]
    };
  },
  set_skill_enabled: function (args) {
    var skillToToggle = findSkillSetting(args.skillId);
    if (!skillToToggle)
      return Promise.reject(new Error("Skill setting not found: " + args.skillId));
    skillToToggle.enabled = args.enabled;
    return null;
  },
  delete_skill_settings: function (args) {
    var skillToDelete = findSkillSetting(args.skillId);
    if (!skillToDelete)
      return Promise.reject(new Error("Skill setting not found: " + args.skillId));
    state.skillSettings = state.skillSettings.filter(function (skill) {
      return skill.settings_id !== skillToDelete.settings_id;
    });
    return null;
  },
  search_remote_skills: function (args) {
    var query = String(args.query || "").toLowerCase();
    return clone(
      state.remoteSkillResults.filter(function (result) {
        return (
          result.name.toLowerCase().indexOf(query) !== -1 ||
          result.description.toLowerCase().indexOf(query) !== -1 ||
          result.package.toLowerCase().indexOf(query) !== -1
        );
      })
    );
  },
  install_remote_skill: function (args) {
    var remoteRequest = args.request;
    var remoteResult = state.remoteSkillResults.find(function (result) {
      return result.package === remoteRequest.package;
    });
    var remoteName = remoteResult ? remoteResult.name : remoteRequest.package;
    var remoteSkill = createSkillSettingFromInstall(
      remoteName,
      remoteRequest.source,
      remoteRequest.target,
      "registry"
    );
    state.skillSettings = state.skillSettings.filter(function (skill) {
      return skill.settings_id !== remoteSkill.settings_id;
    });
    state.skillSettings.push(remoteSkill);
    return clone(remoteSkill);
  },
  install_github_skill: function (args) {
    var githubRequest = args.request;
    var githubName = githubRequest.source.split("/").pop() || "GitHub Skill";
    githubName = githubName.replace(/\.git$/, "").replace(/[-_]+/g, " ");
    var githubSkill = createSkillSettingFromInstall(
      githubName,
      githubRequest.source,
      githubRequest.target,
      "github"
    );
    state.skillSettings = state.skillSettings.filter(function (skill) {
      return skill.settings_id !== githubSkill.settings_id;
    });
    state.skillSettings.push(githubSkill);
    return clone(githubSkill);
  },
  update_skill: function (args) {
    var skillToUpdate = findSkillSetting(args.skillId);
    if (!skillToUpdate)
      return Promise.reject(new Error("Skill setting not found: " + args.skillId));
    skillToUpdate.update_state = "up_to_date";
    return clone(skillToUpdate);
  },
  list_skill_catalog: function (args) {
    var sq = args.query;
    var entries = state.skillCatalog;
    if (sq && sq.keyword) {
      var kw = sq.keyword.toLowerCase();
      entries = entries.filter(function (e) {
        return (
          e.name.toLowerCase().indexOf(kw) !== -1 ||
          e.description.toLowerCase().indexOf(kw) !== -1 ||
          e.package.toLowerCase().indexOf(kw) !== -1
        );
      });
    }
    if (sq && Array.isArray(sq.sources) && sq.sources.length > 0) {
      entries = entries.filter(function (e) {
        return sq.sources.indexOf(e.source) !== -1;
      });
    }
    return clone(entries);
  },
  list_skill_sources: function (args) {
    return state.skillCatalogSources.slice();
  },
  add_skill_source: function (args) {
    var addCfg = args.config;
    state.skillCatalogSources.push(Object.assign({}, addCfg, { last_error: null }));
    return null;
  },
  remove_skill_source: function (args) {
    state.skillCatalogSources = state.skillCatalogSources.filter(function (s) {
      return s.id !== args.id;
    });
    return null;
  },
  set_skill_source_enabled: function (args) {
    state.skillCatalogSources = state.skillCatalogSources.map(function (s) {
      return s.id === args.id ? Object.assign({}, s, { enabled: args.enabled }) : s;
    });
    return null;
  },
  refresh_skill_catalog: function (args) {
    return null;
  }
});
