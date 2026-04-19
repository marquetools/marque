<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00027 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00530" is-a="ValidateTokenValuePrefixesExistenceInList">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00530][Error] The tokens in @ism:SARIdentifier must start with a substring before : that exists
        in the SAR Source Authorities CVE.</sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For any token within @ism:SARIdentifier, invoke ValidateTokenValuePrefixesExistenceInList to check that the token's substring before : 
        exists in the SAR Source Authorities CVE.</sch:p>
    <sch:param name="context" value="*[@ism:SARIdentifier]"/>
    <sch:param name="searchTermList" value="./@ism:SARIdentifier"/>
    <sch:param name="afterText" value="'SAR-'"/>
    <sch:param name="prefix" value="':'"/>
    <sch:param name="list" value="$SARSourceAuthorityList"/>
    <sch:param name="errMsg" value="'[ISM-ID-00530][Error] The tokens in @ism:SARIdentifier must start with a substring before : that exists
        in the SAR Source Authorities CVE.'"/>
</sch:pattern>
