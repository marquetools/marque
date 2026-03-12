<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00384" is-a="DataHasCorrespondingNotice">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00384][Error] If ISM_USGOV_RESOURCE and:
		1. Any element meeting ISM_CONTRIBUTES in the document has the attribute @ism:disseminationControls containing [RS]
		AND
		2. No element meeting ISM_CONTRIBUTES in the document has the attribute @ism:noticeType containing [RSEN, IMCON_RSEN]
		and does not have attribute @ism:externalNotice with a value of [true]
		
		Human Readable: USA documents containing RS data must also have a non-external RSEN or IMCON_RSEN notice.
	</sch:p>
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		This rule uses an abstract pattern to consolidate logic.
		If the document is an ISM_USGOV_RESOURCE, for each element which
		meets ISM_CONTRIBUTES and specifies attribute @ism:disseminationControls
		with a value containing the token [RS], this rule ensures that an element
		meeting ISM_CONTRIBUTES specifies attribute @ism:noticeType with a value
		containing the token [RSEN, IMCON_RSEN] and does not specifiy attribute @ism:externalNotice with a 
		value of [true].
	</sch:p>
	<sch:param name="ruleId" value="'ISM-ID-00384'"/>
	<sch:param name="attrName" value="'disseminationControls'"/>
	<sch:param name="attrValue" value="@ism:disseminationControls"/>
	<sch:param name="noticeType" value="'RSEN', 'IMCON_RSEN'"/>
	<sch:param name="dataType" value="'RS'"/>
</sch:pattern>