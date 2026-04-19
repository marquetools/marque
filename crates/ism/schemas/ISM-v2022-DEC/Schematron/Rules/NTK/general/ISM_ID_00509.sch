<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00509" is-a="ValidateTokenValuesExistenceInList">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00509][Error] ntk:AccessProfileValue with vocabulary role:enterpriseRole must exist in the list of allowed 
        EnterpriseRole values.</sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For ntk:AccessProfileValue whose @ntk:vocabulary is [role:enterpriseRole], invoke ValidateTokenValuesExistenceInList
        to check if the value is in the list of allowed EnterpriseRole values.
    </sch:p>
    <sch:param name="context" value="ntk:AccessProfile/ntk:AccessProfileValue[@ntk:vocabulary='role:enterpriseRole']"/>
    <sch:param name="searchTermList" value="."/>
    <sch:param name="list" value="$entRoleValueList"/>
    <sch:param name="errMsg" value="'[ISM-ID-00509][Error] ntk:AccessProfileValue with vocabulary role:enterpriseRole 
        must exist in the list of allowed EnterpriseRole values.'"/>
</sch:pattern>
