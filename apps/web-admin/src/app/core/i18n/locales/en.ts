/** English dictionary. */
export const EN: Record<string, string> = {
  // Common ---------------------------------------------------------------
  'common.yes': 'Yes',
  'common.no': 'No',
  'common.required': 'Value required.',
  'common.close': 'Close',
  'common.choose': 'Choose...',
  'common.empty': '-',

  // Public portal — chrome ----------------------------------------------
  'public.brand.subtitle': 'Public portal',
  'public.lang.aria': 'Language',
  'public.nav.agent': 'Officer',
  'public.nav.pv': 'Ticket',
  'public.nav.report': 'Report',
  'public.nav.tracking': 'Tracking',
  'public.nav.about': 'About',
  'public.nav.staff': 'Staff sign-in',

  // Why (remark 12) ------------------------------------------------------
  'public.why.agent':
    'Why verify an officer? Make sure someone presenting themselves as a municipal officer is actually registered in the system.',
  'public.why.pv':
    'Why track a ticket? Check the existence and status of a penalty notice handed to you.',
  'public.why.report':
    'Why report? Inform the municipal authorities of an incident observed in your neighbourhood.',

  // Contact / infoline (remarks PP-02, 14bis) ----------------------------
  'public.contact.title': 'Need help?',
  'public.contact.infoline': 'Infoline',
  'public.contact.whatsapp': 'WhatsApp',

  // Officer / ticket verification ---------------------------------------
  'public.verify.eyebrow': 'Public verification',
  'public.verify.agent.title': 'Verify an officer',
  'public.verify.pv.title': 'Verify a ticket',
  'public.verify.subtitle':
    'Displayed data is limited to information useful for public verification.',
  'public.verify.agent.label': 'Officer ID',
  'public.verify.pv.label': 'Ticket number',
  'public.verify.submit': 'Verify',
  'public.verify.agent.notFound': 'The ID entered does not match any record in the database.',
  'public.verify.pv.notFound':
    'The ticket number entered does not match any record in the database.',
  'public.verify.agent.photoAlt': 'Officer photo',
  'public.verify.agent.help':
    "Enter the ID shown on the officer's professional card to confirm their identity.",
  'public.verify.pv.help':
    'Enter the penalty notice number (PV-…) to check its existence and status.',

  // Report ---------------------------------------------------------------
  'public.report.eyebrow': 'Citizen report',
  'public.report.title': 'Submit a report',
  'public.report.subtitle': 'A tracking number is generated after validation.',
  'public.report.help':
    'A report is a complaint against an officer. Provide the location (region, department, commune), the type of contested action and the officer involved, then describe what happened. Contact details are optional.',
  'public.report.region': 'Region',
  'public.report.departement': 'Department',
  'public.report.commune': 'Commune',
  'public.report.category': 'Report category',
  'public.report.type': 'Type of contested action',
  'public.report.type.amende': 'Fine',
  'public.report.type.verbalisation': 'Ticketing',
  'public.report.type.scelle': 'Seizure (sealing)',
  'public.report.type.fourriere': 'Vehicle impound',
  'public.report.type.autre': 'Other',
  'public.report.reportedAgentMat': 'Reported officer ID',
  'public.report.reportedAgentName': 'Reported officer name',
  'public.report.incidentDate': 'Incident date and time',
  'public.report.pvRef': 'Related ticket number',
  'public.report.zone': 'Neighbourhood / zone',
  'public.report.lieuDit': 'Place name',
  'public.report.description': 'Description',
  'public.report.anonymous': 'Stay anonymous',
  'public.report.contactName': 'Contact name',
  'public.report.contactPhone': 'Phone / WhatsApp',
  'public.report.submit': 'Send',
  'public.report.submitting': 'Sending...',
  'public.report.newReport': 'New report',
  'public.report.tracking': 'Tracking number: {value}',
  'public.report.successNote':
    'Keep this number to track your report. If you provided contact details, you may be contacted via WhatsApp.',
  'public.report.error': 'Report rejected. Please check the information entered.',
  'public.report.communesError': 'Unable to load the available communes.',
  'public.report.optionsError': 'Options unavailable for this commune.',
  'public.report.geoError': 'Unable to load the available geography.',

  // Report tracking ------------------------------------------------------
  'public.track.eyebrow': 'Public tracking',
  'public.track.title': 'Track a report',
  'public.track.subtitle': 'Public tracking exposes neither administrative notes nor contacts.',
  'public.track.label': 'Tracking number',
  'public.track.submit': 'Look up',
  'public.track.required': 'Number required.',
  'public.track.notFound': 'The code entered does not match any report number in the database.',
  'public.track.help':
    'Enter the tracking number (SIG-…) received at submission to check progress.',

  // About (remark PP-04) -------------------------------------------------
  'public.about.eyebrow': 'About',
  'public.about.title': 'About APMTRACK',
  'public.about.intro':
    'APMTRACK is the management platform for Municipal Police Officers (APM). It equips communes to track field officers, issue and collect penalty notices, and process citizen reports.',
  'public.about.missionTitle': 'Our mission',
  'public.about.missionBody':
    'Strengthen the transparency and efficiency of the municipal police by offering citizens simple tools: verify an officer, check a penalty notice and report an incident.',
  'public.about.rolesTitle': 'Roles and missions',
  'public.about.rolesBody':
    'APM officers ensure compliance with municipal by-laws, public sanitation and local public order. Every action is traced and linked to a commune.',

  // Public field labels --------------------------------------------------
  'field.matricule': 'ID',
  'field.agent_matricule': 'Issued by',
  'field.full_name': 'Name',
  'field.status': 'Status',
  'field.active': 'Active',
  'field.commune_nom': 'Commune',
  'field.pv_number': 'Ticket number',
  'field.amount_initial_fcfa': 'Amount',
  'field.signalement_number': 'Tracking number',
  'field.type_incident': 'Incident type',
  'field.created_at': 'Created on',
  'field.updated_at': 'Updated on',
};
