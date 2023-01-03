<template>
  <div id="wrapper" class="wrapper" :class="{
    dark: darkTheme,
  }">
    <nav class="uk-navbar-container uk-margin" uk-navbar>
      <div class="uk-navbar-left">
        <ul class="uk-navbar-nav">
          <li>
            <router-link :to="{ name: 'Home' }">Home</router-link>
          </li>
          <li>
            <router-link :to="{ name: 'Control' }">Control</router-link>
          </li>
          <li><a href="#">Live</a></li>
          <div class="uk-navbar-dropdown">
            not implemented!
          </div>
          <li><a href="/explore" target="_blank">Exlpore</a></li>
        </ul>
      </div>
      <div class="uk-navbar-right">
        <ul class="uk-navbar-nav">

          <li>
            <div class="uk-navbar-item">
              <label class="switch">
                <input type="checkbox" v-model="darkTheme" @change="themeChanged">
                <span class="slider round"></span>
              </label>
              <!--
              <div class="moon-container">
                <img src="../public/fav/half-moon.png" alt="moon">
              </div>
              -->
            </div>
          </li>
          <li>
            <div class="uk-navbar-item">
              <span uk-icon="users"></span>
              <label>{{ connectedClientsCount }}</label>
            </div>
          </li>
          <li>
            <div class="uk-navbar-item">
              <span uk-icon="play"></span>
              <label>{{ runningTestsCount }}</label>
            </div>
          </li>
          <li>
            <div class="uk-navbar-item">
              <span uk-icon="download"></span>
              <div v-if="showInstallingProjects" class="uk-navbar-dropdown">
                <ul class="uk-nav uk-navbar-dropdown-nav uk-list-divider">
                  <li v-for="project in installingProjects" :key="project">
                    {{ project }}
                  </li>
                </ul>
              </div>
              <label>{{ installingProjects.length }}</label>
            </div>
          </li>
        </ul>
      </div>
    </nav>
    <div class="content">
      <router-view :newProject="newProject" :deletedProject="deletedProject" :darkTheme="darkTheme" />
    </div>
  </div>

</template>

<script>

export default {
  name: "App",
  data() {
    return {
      ws: null,
      connectedClientsCount: 0,
      runningTestsCount: 0,
      installingProjects: [],
      showInstallingProjects: false,
      newProject: null,
      deletedProject: null,
      darkTheme: false
    };
  },
  methods: {
    themeChanged() {
      localStorage.setItem("darkTheme", this.darkTheme);
      this.handleDarkTheme()
    },
    handleDarkTheme() {
      if (this.darkTheme) {
        this.setBodyDark();
      } else {
        this.removeBodyDark();
      }
    },
    setBodyDark() {
      const body = document.body
      body.classList.add('dark')
    },
    removeBodyDark() {
      const body = document.body
      body.classList.remove('dark')
    },
    notification(text, status, timeout) {
      UIkit.notification(text, {
        status: status,
        pos: "bottom-right",
        timeout: timeout,
      });
    },
    connenctWebsocket() {
      var wsProtocol = "ws";
      if (location.protocol == 'https:') {
        wsProtocol = "wss";
      }

      this.ws = new WebSocket(`${wsProtocol}://${location.host}/api/ws`);
      this.ws.onopen = () => { };
      this.ws.onclose = () => { };
      this.ws.onmessage = (event) => {
        console.log(event.data);
        const data = JSON.parse(event.data);
        const event_type = data.event_type;
        if (event_type === "INFORMATION") {
          this.connectedClientsCount = data.event.connected_clients_count;
          this.runningTestsCount = data.event.running_tests_count;
          this.installingProjects = data.event.istalling_projects;
          if (this.installingProjects.length > 0) {
            this.showInstallingProjects = true;
            return;
          }
          this.showInstallingProjects = false;
          return;
        }
        if (event_type === "PROJECTS") {
          const istalling_projects = data.event.istalling_projects;
          for (var i = 0; i < istalling_projects.length; i++) {
            let project = istalling_projects[i];
            if (project.status === 1) {
              //get scripts
              fetch(`/api/project/${project.id}`)
                .then((data) => data.json())
                .then((data) => {
                  if (data.success) {
                    const scripts = data.content.scripts;
                    this.newProject = { id: project.id, scripts: scripts };
                  }
                })
                .catch();
              //notify
              this.notification(
                `Project: [${project.id}] installed successfully`,
                "primary",
                10000
              );
              return;
            }
            if (project.status === 2) {
              const error = project.error;
              //notify
              this.notification(
                `Error installing project: [${project.id}]: ${error}`,
                "danger",
                0
              );
              return;
            }
          }
          //{"event_type":"PROJECTS","event":{"istalling_projects":[{"id":"Neuer_Ordner-Kopie","status":0,"error":null}]}}
          return;
        }
        if (event_type === "PROJECT_DELETED") {
          const project_id = data.event.id;
          //notify
          this.notification(
            `Project: [${project_id}] was deleted`,
            "primary",
            10000
          );
          this.deletedProject = { id: project_id };
        }
      };
    },
  },
  created() {
    this.darkTheme = JSON.parse(localStorage.getItem("darkTheme"));
    this.handleDarkTheme();
    this.connenctWebsocket();
  }

};
</script>

<style>

</style>
