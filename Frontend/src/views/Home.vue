<template>
  <div>
    <div class="uk-grid-small uk-child-width-1-2@s uk-flex-center uk-text-center" uk-grid>
      <div class="uk-width-auto@m">
        <button class="uk-button uk-button-primary" type="button" uk-toggle="target: #upload-modal"
          @click="uploadMessage = ''">
          Upload
        </button>
      </div>
      <div class="uk-width-auto@m">
        <button class="uk-button uk-button-danger" type="button" @click="deleteProjects">
          Delete
        </button>
      </div>
    </div>
    <div id="upload-modal" uk-modal ref="upload-modal">
      <div class="uk-modal-dialog uk-modal-body" :class="{
        dark: darkTheme,
      }">
        <form>
          <div class="uk-margin" uk-margin>
            <h2 class="text">Upload a new project</h2>
            <div class="upload-container">
              <div uk-form-custom="target: true">
                <input type="file" webkitdirectory mozdirectory ref="files" />
                <input class="uk-input uk-form-width-medium" type="text" placeholder="Select project" disabled />
              </div>
              <button type="button" class="uk-button uk-button-primary" @click="upload">
                Upload
              </button>
              <div v-if="uploading" uk-spinner class="upload-spinner"></div>
            </div>
            <progress class="uk-progress" v-if="uploading" max="100" :value="percentCompleted"> </progress>
            <h5>{{ uploadMessage }}</h5>
          </div>
        </form>
      </div>
    </div>

    <h1 class="text">Projects</h1>
    <div class="uk-grid uk-flex-center uk-text-center" uk-grid>
      <div v-for="project in projects" :key="project.id" v-motion :initial="{
        opacity: 0,
        x: 50,
      }" :enter="{
        opacity: 1,
        x: 0,
      }">
        <div class="uk-card uk-card-default uk-width-auto@m uk-card-body">
          <label class="checkbox-label">
            <input type="checkbox" class="checkbox-input" :value="project.id" v-model="projectsToBeDeleted" />
            <span class="checkbox"> </span>
          </label>
          <h3 class="uk-card-title text navigation-h" @click="navigateToProject(project.id)">{{ project.id }}</h3>
          <ul class="uk-list uk-list-divider script-list">
            <li v-for="script in project.scripts" :key="script">
              <router-link :to="{
                name: 'Script',
                params: { pid: project.id, id: script },
              }">
                {{ script }}
              </router-link>
            </li>
          </ul>
        </div>
      </div>
    </div>
    <!-- <ul class="uk-list">
      <li v-for="project in projects" :key="project.id" v-motion :initial="{
        opacity: 0,
        x: 50,
      }" :enter="{
        opacity: 1,
        x: 0,
      }">
        <div class="uk-card uk-card-default uk-card-body">
          <label class="checkbox-label">
            <input type="checkbox" class="checkbox-input" :value="project.id" v-model="projectsToBeDeleted" />
            <span class="checkbox"> </span>
          </label>
          <h3 class="uk-card-title text">{{ project.id }}</h3>
          <ul class="uk-list uk-list-divider script-list">
            <li v-for="script in project.scripts" :key="script">
              <router-link :to="{
                name: 'Script',
                params: { pid: project.id, id: script },
              }">
                {{ script }}
              </router-link>
            </li>
          </ul>
        </div>
      </li>
    </ul> -->
    <br />
    <br />
  </div>
</template>

<script>
export default {
  name: "Home",
  props: ["newProject", "deletedProject", "darkTheme"],
  watch: {
    newProject: function () {
      this.projects.push(this.newProject);
    },
    deletedProject: function () {
      this.projects = this.projects.filter(
        (project) => project.id !== this.deletedProject.id
      );
    },
  },
  data() {
    return {
      uploading: false,
      projects: [],
      uploadMessage: "",
      projectsToBeDeleted: [],
      percentCompleted: 0,
    };
  },
  methods: {
    navigateToProject(pid) {
      this.$router.push({ name: 'Project', params: { pid: pid } });
    },
    getProjects() {
      fetch("/api/projects")
        .then((data) => data.json())
        .then((data) => {
          this.projects = data.content.projects;
        })
        .catch();
    },
    hideUploadModal() {
      UIkit.modal(this.$refs["upload-modal"]).hide();
    },
    upload() {
      const files = this.$refs.files.files;
      console.log(files);
      if (files.length < 1) {
        this.uploadMessage = "Please select a directory to upload";
        return false;
      }
      var data = new FormData();
      for (var i = 0; i < files.length; i++) {
        data.append("file" + i, files[i]);
      }
      this.uploading = true;

      axios
        .request({
          method: "post",
          url: "/api/upload",
          data: data,
          onUploadProgress: (progressEvent) => {
            this.percentCompleted = Math.round(
              (progressEvent.loaded * 100) / progressEvent.total
            );
            console.log(this.percentCompleted);
          },
        })
        .then((response) => {
          this.uploading = false;
          const data = response.data;
          this.uploadResponse = data;
          this.percentCompleted = 0;
          if (data.success) {
            this.hideUploadModal();
          } else {
            this.uploadMessage = data.error;
          }
        })
        .catch(() => {
          this.uploading = false;
          this.percentCompleted = 0;
          this.hideUploadModal();
        });

      // fetch("/api/upload", {
      //   method: "POST",
      //   body: data,
      // })
      //   .then((data) => data.json())
      //   .then((data) => {
      //     console.log(data);
      //     this.uploading = false;
      //     if (data.success) {
      //       this.hideUploadModal();
      //     } else {
      //       this.uploadMessage = data.error;
      //     }
      //   })
      //   .catch(() => {
      //     this.uploading = false;
      //     this.hideUploadModal();
      //   });

      return false;
    },
    deleteProjects() {
      fetch(`/api/delete_projects`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ project_ids: this.projectsToBeDeleted }),
      })
        .then((data) => data.json())
        .then((data) => {
          this.projectsToBeDeleted = [];
          if (data.success) {
          } else {
            console.log(data.error);
          }
        })
        .catch(() => {
          this.projectsToBeDeleted = [];
        });

      return false;
    },
  },
  created() {
    this.getProjects();
  },
};
</script>
