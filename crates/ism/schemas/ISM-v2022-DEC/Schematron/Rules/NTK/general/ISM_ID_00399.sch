<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00051 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00399" is-a="ValueExistsInList">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00399][Error] If a profile DES URN begins with ‘urn:us:gov:ic:ntk:’, the value
        must exist in the list of allowed values.</sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If ProfileDes starts with 'urn:us:gov:ic:ntk', invoke abstract rule ValueExistsInList to check
        if the value exists in the NTKProfileDes CVE.</sch:p>
    <sch:param name="context" value="ntk:ProfileDes[starts-with(., 'urn:us:gov:ic:ntk:')]"/>
    <sch:param name="list" value="$profileDESList"/>
    <sch:param name="errMsg" value="'[ISM-ID-00399][Error] Profile DES URNs that start with IC CIO reserved portion must exist in NTKProfileDes CVE.'"/>
</sch:pattern>
