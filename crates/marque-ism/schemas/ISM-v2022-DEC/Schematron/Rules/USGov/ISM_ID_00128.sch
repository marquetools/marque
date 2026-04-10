<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00128" is-a="DataHasCorrespondingNoticeWithException">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00128][Error] USA documents containing FRD data must have a
        non-external FRD notice unless the document contains RD in the banner.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        This rule depends on the DataHasCorrespondingNoticeWithException
        abstract pattern to enforce that FRD documents have an FRD notice unless the banner has RD.
        See DataHasCorrespondingNoticeWithException for details.
    </sch:p>
    <sch:param name="ruleId" value="'ISM-ID-00128'"/>
    <sch:param name="attrName" value="'atomicEnergyMarkings'"/>
    <sch:param name="attrValue" value="$ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings"/>
    <sch:param name="noticeType" value="'FRD'"/>
    <sch:param name="exceptAttrValue" value="$ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings"/>
    <sch:param name="exceptNoticeType" value="'RD'"/>
</sch:pattern>