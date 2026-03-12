<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00027 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00420" is-a="ValidateTokenValuesExistenceInList">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00420][Error] organization:usa-agency vocabulary values must exist in the USAgency CVE.</sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For ntk:AccessProfileValue with organization 'usa-agency' invoke ValueExistsInList to check 
        if the value exists in the USAgency CVE.</sch:p>
    <sch:param name="context" value="ntk:AccessProfile/ntk:AccessProfileValue[@ntk:vocabulary='organization:usa-agency']"/>
    <sch:param name="searchTermList" value="."/>
    <sch:param name="list" value="$usagencyList"/>
    <sch:param name="errMsg" value="'[ISM-ID-00420][Error] organization:usa-agency vocabulary values must exist in the USAgency CVE.'"/>
</sch:pattern>
