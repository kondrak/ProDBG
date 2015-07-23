#include "pd_view.h"
#include "pd_backend.h"
#include "pd_host.h"
#include <stdlib.h>
#include <stdio.h>

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static PDDialogFuncs* s_dialogFuncs;

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

struct Entry
{
	const char* name;
	bool isDirectory;
};

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

struct TreeEntry
{
	int count;
	bool* foldedState;
	Entry* entries;
};

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

struct WorkspaceData
{
	int dummy;
};

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static int update(void* userData, PDUI* uiFuncs, PDReader* reader, PDWriter* writer)
{
    uint32_t event = 0;

    (void)userData;
    (void)uiFuncs;
    (void)reader;
    (void)writer;

    if (uiFuncs->button("OpenDialog", (PDVec2) { 0.0f, 0.0f }))
	{
		char outputPath[4096];
		s_dialogFuncs->selectDirectory(outputPath);
	}

    while ((event = PDRead_getEvent(reader)) != 0)
    {
    	(void)event;
    }

    return 0;
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static void* createInstance(PDUI* uiFuncs, ServiceFunc* serviceFunc)
{
    (void)serviceFunc;
    (void)uiFuncs;
    WorkspaceData* userData = (WorkspaceData*)malloc(sizeof(WorkspaceData));

	s_dialogFuncs = (PDDialogFuncs*)serviceFunc(PDDIALOGS_GLOBAL);

    return userData;
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static void destroyInstance(void* userData)
{
    free(userData);
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static PDViewPlugin plugin =
{
    "Workspace",
    createInstance,
    destroyInstance,
    update,
};

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

extern "C"
{

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

PD_EXPORT void InitPlugin(RegisterPlugin* registerPlugin, void* privateData)
{
	registerPlugin(PD_VIEW_API_VERSION, &plugin, privateData);
}

}


