<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00487" is-a="DataHasCorrespondingNotice">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00487][Error] If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and:
        1. Any element meeting ISM_CONTRIBUTES in the document has the attribute @ism:cuiSpecified containing [FISA]
        AND
        2. No element meeting ISM_CONTRIBUTES in the document has the attribute @ism:noticeType containing [FISA]
        and does not have attribute @ism:externalNotice with a value of [true].
        
        Human Readable: USA documents containing FISA data in ism:cuiSpecified must also have a non-external FISA notice.
    </sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		This rule uses an abstract pattern to consolidate logic.
		If the document is an ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, for each element which
		meets ISM_CONTRIBUTES and specifies attribute @ism:cuiSpecified
		with a value containing the token [FISA], this rule ensures that an element
		meeting ISM_CONTRIBUTES specifies attribute @ism:noticeType with a value
		containing the token [FISA] and does not specifiy attribute @ism:externalNotice with a value of [true].
	</sch:p>
	  <sch:param name="ruleId" value="'ISM-ID-00487'"/>
	  <sch:param name="attrName" value="'cuiSpecified'"/>
	  <sch:param name="attrValue" value="@ism:cuiSpecified"/>
	  <sch:param name="noticeType" value="'FISA'"/>
	  <sch:param name="dataType" value="'FISA'"/>
</sch:pattern>