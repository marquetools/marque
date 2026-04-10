<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00356" is-a="DataHasCorrespondingNotice">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00356][Error] USA documents containing SSI data must have a non-external SSI notice.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	This rule uses an abstract pattern to enforce that an appropriate notice exists for SSI information.
    	
    	If (1) the document is an ISM_USGOV_RESOURCE and (2) the document contains an element that
    	contributes to rollup (ISM_CONTRIBUTES is true) that also has an ism:nonICmarkings attribute
    	that contains the token [SSI], there must exist an element that contributes to rollup
    	(ISM_CONTRIBUTES is true) that has an ism:noticeType attribute containing the token [SSI]
    	and that does not have an ism:externalNotice attribute with a value of [true].
    </sch:p>
	  <sch:param name="ruleId" value="'ISM-ID-00356'"/>
      <sch:param name="attrName" value="'ism:nonICmarkings'"/>
      <sch:param name="attrValue" value="@ism:nonICmarkings"/>
	  <sch:param name="noticeType" value="'SSI'"/>
      <sch:param name="dataType" value="'SSI'"/>
</sch:pattern>